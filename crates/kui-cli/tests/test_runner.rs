//! Exercise the shipped binary across process, Lua and renderer seams.
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
const KUI: &str = env!("CARGO_BIN_EXE_kui");
fn app(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (path, text) in files {
        let p = dir.path().join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    if !dir.path().join("main.lua").exists() {
        std::fs::write(
            dir.path().join("main.lua"),
            "function view() return column { text('fixture') } end",
        )
        .unwrap();
    }
    std::fs::write(
        dir.path().join("kui.config.lua"),
        "return {name='test fixture',entry='main.lua',size={w=360,h=240}}",
    )
    .unwrap();
    dir
}
fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(KUI)
        .current_dir(root)
        .arg("test")
        .args(args)
        .output()
        .unwrap()
}
fn text(out: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}
fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let target = to.join(e.file_name());
        if e.path().is_dir() {
            copy(&e.path(), &target)
        } else {
            std::fs::copy(e.path(), target).unwrap();
        }
    }
}
#[test]
fn app_cases_drive_input_virtual_time_and_native_effects_and_write_portable_artifacts() {
    let root = tempfile::tempdir().unwrap();
    copy(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/test-runner"),
        root.path(),
    );
    let output = run(root.path(), &["--reporter", "junit"]);
    assert!(output.status.success(), "{}", text(&output));
    let folder = root.path().join("test-results");
    let results: serde_json::Value =
        serde_json::from_slice(&std::fs::read(folder.join("results.json")).unwrap()).unwrap();
    assert_eq!(results["cases"].as_array().unwrap().len(), 4);
    let xml = std::fs::read_to_string(folder.join("junit.xml")).unwrap();
    assert!(xml.contains("tests=\"4\"") && xml.contains("failures=\"0\""));
    let events = std::fs::read_to_string(latest_file(root.path()).join("events.jsonl")).unwrap();
    assert!(events.contains("http") && events.contains("assertion") && events.contains("message"));
    let frame: serde_json::Value = serde_json::from_slice(
        &std::fs::read(latest_file(root.path()).join("frames/1-1.json")).unwrap(),
    )
    .unwrap();
    assert!(frame["drawing"].as_str().unwrap().starts_with("blobs/"));
    assert!(frame["model_blob"].as_str().unwrap().starts_with("blobs/"));
    assert!(frame["kit_blob"].as_str().unwrap().starts_with("blobs/"));
}
#[test]
fn assertions_hooks_spies_graph_snapshots_and_filter_work_without_an_app() {
    let root = app(&[
        (
            "test/main.lua",
            r#"
local t=require('kui.test')
local before,after=0,0
t.before_each(function() before=before+1 end)
t.after_each(function() after=after+1 end)
t.describe('values',function()
  t.it('graphs',function()
    t.expect(before):to_be(1)
    local shared={n=7};local value={shared, alias=shared, [7]='mixed'};value.self=value
    t.expect_table('graph',value)
    t.expect({a={1,2}}):to_equal({a={1,2}})
    t.expect('abc').never:to_contain('z')
    t.expect(1.01):to_be_near(1,0.02)
    t.expect(function() error('boom') end):to_throw('boom')
    local object={fn=function(a,...) return a,nil,... end};local spy=t.spy(object,'fn')
    local a,b,c=object.fn('x','tail');t.expect(a):to_be('x');t.expect(b):to_be(nil);t.expect(c):to_be('tail')
    t.expect(spy):to_have_been_called_with('x','tail')
  end)
  t.it('other',function() error('filter should skip this') end)
end)
"#,
        ),
        (
            "test/other.lua",
            "local t=require('kui.test');t.it('unselected',function() error('filtered') end)",
        ),
    ]);
    let first = run(root.path(), &["--filter", "values / graphs", "--update"]);
    assert!(first.status.success(), "{}", text(&first));
    let second = run(root.path(), &["--filter", "values / graphs"]);
    assert!(second.status.success(), "{}", text(&second));
    assert!(text(&second).contains("1 passed, 0 failed"));
    let snapshot = std::fs::read_dir(root.path().join("test/__snapshots__"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let graph: serde_json::Value =
        serde_json::from_slice(&std::fs::read(snapshot).unwrap()).unwrap();
    assert_eq!(graph["version"], 1);
    assert!(
        graph["objects"]
            .as_array()
            .unwrap()
            .iter()
            .any(|o| o["entries"].as_array().is_some_and(|v| v.len() == 4))
    );
}
#[test]
fn file_failures_and_timeouts_do_not_hide_other_files_and_teal_twins_run_once() {
    let root = app(&[
        (
            "test/a.lua",
            "local t=require('kui.test');t.it('fails',function() t.expect(1):to_be(2) end)",
        ),
        ("test/b.lua", "while true do end"),
        (
            "test/c.tl",
            "local t = require('kui.test')\nt.it('teal survives', function() t.expect(2):to_be(2) end)",
        ),
        ("test/c.lua", "error('generated twin must not run')"),
        ("test/types.d.tl", "this is a declaration, not executable"),
        ("test/fixtures/helper.lua", "error('fixture must not run')"),
    ]);
    let output = run(root.path(), &["--timeout", "1000", "--reporter", "junit"]);
    assert!(!output.status.success());
    let log = text(&output);
    assert!(
        log.contains("expected 1 to be 2")
            && log.contains("timed out")
            && log.contains("teal survives"),
        "{log}"
    );
    assert!(log.contains("1 passed, 2 failed"), "{log}");
    assert!(!log.contains("generated twin must not run"));
    let report: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.path().join("test-results/report.json")).unwrap(),
    )
    .unwrap();
    assert!(report["files"].as_array().unwrap().iter().any(|f| {
        f["file"].as_str().is_some_and(|s| s.ends_with("b.lua"))
            && f["diagnostics"].as_array().is_some_and(|d| {
                d.iter().any(|v| {
                    v["message"]
                        .as_str()
                        .is_some_and(|s| s.contains("timed out"))
                })
            })
    }));
}
#[test]
fn mock_rules_keep_error_codes_process_streams_and_worker_inheritance() {
    let root = app(&[
        (
            "worker.lua",
            r#"function on_message() local r=kui.http.get('https://worker.test');kui.dispatch({kind='reply',body=r.body}) end"#,
        ),
        (
            "main.lua",
            r#"
function init() return {reply=nil} end
function update(msg)
 if msg.kind=='reply' then model.reply=msg.body
 elseif msg.kind=='worker' then local w=kui.worker('worker.lua');w:dispatch({}) end
end
function view() return column {text('fixture')} end
"#,
        ),
        (
            "test/main.lua",
            r#"
local t=require('kui.test')
t.it('native mocks',function()
  t.mock_http {url='https://worker.test',response={body='worker mocked'}}
  t.mock_process {op='spawn',command='imaginary-tool',response={stdout={{text='first',at=10},{text='last',at=20}},stderr='warning',code=7},delay=30}
  t.start()
  t.dispatch {kind='worker'}
  t.wait_for(function() return t.model().reply~=nil end)
  t.expect(t.model().reply):to_be('worker mocked')
  t.expect(t.eval([[local p=kui.process.spawn('imaginary-tool');assert(p:read_line()=='first');assert(p:read_line()=='last');assert(p:read_line()==nil);assert(p:read_line('stderr')=='warning');p:write('stdin');p:close();local ok,_,code=p:try_write('closed');assert(not ok and code=='closed');return p:wait()]])):to_be(7)
  t.expect(t.eval([[local ok,_,code=kui.http.try_get('https://unexpected.test');return not ok and code=='unexpected_call']])):to_be(true)
  t.mock_fs {op='write',error='disk full',code='full',passthrough=true}
  t.expect(t.eval([[local ok,_,code=kui.fs.try_write(kui.data_dir..'/x','text');return not ok and code=='full']])):to_be(true)
  t.expect(t.eval([[kui.secrets.set('token','value');return kui.secrets.get('token')]])):to_be('value')
end)
"#,
        ),
    ]);
    let output = run(root.path(), &[]);
    assert!(output.status.success(), "{}", text(&output));
}

#[test]
fn debugger_pauses_at_actions_steps_frames_and_resumes_the_same_test() {
    let root = app(&[
        (
            "main.lua",
            "function init() return {n=0} end;function update() model.n=model.n+1 end;function view() return column{text(tostring(model.n))} end",
        ),
        (
            "test/main.lua",
            "local t=require('kui.test');t.it('debuggable',function()t.start();t.dispatch({});t.expect(t.model().n):to_be(1) end)",
        ),
    ]);
    let output = root.path().join("artifacts");
    let file = output.join("file-0000");
    std::fs::create_dir_all(&file).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let options = serde_json::json!({"root":root.path(),"output":output,"filter":null,"update":false,"system_fonts":false,"headed":false,"debug":true,"ui":true,"every_frame":false,"timeout":3000});
    std::fs::write(output.join("options.json"), options.to_string()).unwrap();
    let child = Command::new(KUI)
        .arg("__test-file")
        .arg(output.join("options.json"))
        .arg(root.path().join("test/main.lua"))
        .arg(&file)
        .arg(tmp.path())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    struct OwnedChild(std::process::Child);
    impl Drop for OwnedChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut child = OwnedChild(child);
    let wait = |predicate: &dyn Fn() -> bool| {
        let start = std::time::Instant::now();
        while !predicate() {
            assert!(
                start.elapsed().as_secs() < 8,
                "debug controller did not reach its checkpoint"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    };
    let events = || std::fs::read_to_string(file.join("events.jsonl")).unwrap_or_default();
    wait(&|| events().contains("\"phase\":\"before\""));
    wait(&|| {
        std::fs::read_to_string(file.join("status.json"))
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .is_some_and(|s| s["paused"] == true)
    });
    assert!(
        file.join("frames/1-1.json").is_file(),
        "Debug paused without launching the app or capturing its initial frame"
    );
    let pause: serde_json::Value =
        serde_json::from_slice(&std::fs::read(file.join("status.json")).unwrap()).unwrap();
    assert_eq!(pause["app_ready"], true);

    std::fs::create_dir_all(file.join("commands")).unwrap();
    std::fs::write(file.join("commands/1.json"), r#"{"command":"next"}"#).unwrap();
    wait(&|| events().contains("\"name\":\"dispatch\""));
    wait(&|| {
        events()
            .lines()
            .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
            .any(|e| {
                e["kind"] == "step"
                    && e["data"]["name"] == "dispatch"
                    && e["data"]["phase"] == "after"
            })
    });

    std::fs::write(file.join("commands/2.json"), r#"{"command":"frame"}"#).unwrap();
    wait(&|| events().contains("debug frame"));
    std::fs::write(file.join("commands/3.json"), r#"{"command":"resume"}"#).unwrap();
    wait(&|| file.join("results.json").exists());
    let status = child.0.wait().unwrap();
    assert!(status.success());
    let result: serde_json::Value =
        serde_json::from_slice(&std::fs::read(file.join("results.json")).unwrap()).unwrap();
    assert_eq!(result["cases"][0]["ok"], true);
}

#[test]
fn mock_http_matches_native_response_fields_and_rejects_invalid_schedules() {
    let root = app(&[(
        "test/main.lua",
        r#"
local t=require('kui.test')
t.it('response protocol',function()
  t.mock_http {url='https://json.test',response={status=201,json={answer=42}}}
  t.mock_http {url='https://body.test',response={status=404,body='{"missing":true}',headers={['Content-Type']='application/json'}}}
  t.mock_process {command='failing-tool',response={code=7}}
  t.expect(function() t.mock_process {delay=-1} end):to_throw('nonnegative')
  t.expect(function() t.mock_http {progress={{at=-1}}} end):to_throw('nonnegative')
  t.start()
  t.expect(t.eval([[local r=kui.process.run('failing-tool');return not r.ok and r.code==7 and r.stdout=='' and r.stderr=='']])):to_be(true)
  t.expect(t.eval([[local r=kui.http.get('https://json.test');return r.status==201 and r.ok and r.json.answer==42 and r.body=='{"answer":42}']])):to_be(true)
  t.expect(t.eval([[local r=kui.http.get('https://body.test');return r.status==404 and not r.ok and r.json.missing]])):to_be(true)
end)
"#,
    )]);
    let output = run(root.path(), &[]);
    assert!(output.status.success(), "{}", text(&output));
}

#[test]
fn temporary_resources_clean_up_and_explicit_keep_survives_case_and_parent_teardown() {
    let root = app(&[(
        "test/main.lua",
        r#"
local t=require('kui.test')
t.it('temporary lifetime',function()
  local dir=kui.fs.temp_dir()
  local kept=kui.fs.temp_file {dir=dir,keep=true,prefix='kept-'}
  local gone=kui.fs.temp_file {dir=dir,prefix='gone-'}
  kui.fs.write(kept,'keep')
  kui.fs.write('paths.txt',kept..'\n'..gone)
end)
"#,
    )]);
    let output = run(root.path(), &[]);
    assert!(output.status.success(), "{}", text(&output));
    let paths = std::fs::read_to_string(root.path().join("paths.txt")).unwrap();
    let mut paths = paths.lines();
    let kept = PathBuf::from(paths.next().unwrap());
    let gone = PathBuf::from(paths.next().unwrap());
    assert_eq!(std::fs::read_to_string(&kept).unwrap(), "keep");
    assert!(!gone.exists());
    std::fs::remove_file(&kept).unwrap();
    std::fs::remove_dir(kept.parent().unwrap()).unwrap();
}

#[test]
fn snapshot_failures_keep_portable_text_and_pixel_diffs() {
    let root = app(&[
        (
            "main.lua",
            "function view() return column{ text('before') } end",
        ),
        (
            "test/main.lua",
            r#"
local t=require('kui.test')
t.it('pixels',function()t.start();t.expect_shot('view') end)
t.it('tree',function()t.start();t.expect_tree('view') end)
"#,
        ),
    ]);
    let baseline = run(root.path(), &["--update"]);
    assert!(baseline.status.success(), "{}", text(&baseline));
    std::fs::write(
        root.path().join("main.lua"),
        "function view() return column{ text('after') } end",
    )
    .unwrap();
    let output = run(root.path(), &[]);
    assert!(!output.status.success(), "{}", text(&output));
    assert!(text(&output).contains("0 passed, 2 failed"));
    let report: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.path().join("test-results/report.json")).unwrap(),
    )
    .unwrap();
    let failures = report["files"][0]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["kind"] == "test_failure")
        .collect::<Vec<_>>();
    assert_eq!(failures[0]["source"]["line"], 3);
    assert_eq!(failures[1]["source"]["line"], 4);
    let dir = latest_file(root.path());
    let log = std::fs::read_to_string(dir.join("events.jsonl")).unwrap();
    let snapshots = log
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter(|e| e["kind"] == "snapshot")
        .collect::<Vec<_>>();
    assert_eq!(snapshots.len(), 2);
    for event in snapshots {
        assert_eq!(event["data"]["ok"], false);
        for key in ["expected", "actual", "diff"] {
            assert!(
                dir.join(event["data"][key].as_str().unwrap()).is_file(),
                "missing {key}"
            );
        }
    }
}

#[test]
fn ordinary_apps_release_temporary_paths_and_teal_tests_type_check() {
    let root = app(&[
        (
            "main.lua",
            r#"
function init()
  local p=kui.fs.temp_file();kui.fs.write('temporary-path.txt',p)
  kui.exit(0);return {}
end
function view() return column{} end
"#,
        ),
        (
            "test/main.tl",
            r#"
local t = require('kui.test')
t.it('typed', function()
 local app = t.start {args={},size={w=320,h=200}}
 app:frames(1)
 t.expect(1):to_be(1)
 t.wait_for(function(): boolean return true end)
end)
"#,
        ),
    ]);
    let output = Command::new(KUI)
        .args(["run", "--headless", "--no-rename"])
        .current_dir(root.path())
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", text(&output));
    let temporary = std::fs::read_to_string(root.path().join("temporary-path.txt")).unwrap();
    assert!(
        !Path::new(&temporary).exists(),
        "ordinary app retained {temporary}"
    );
    let checked = Command::new(KUI)
        .arg("check")
        .current_dir(root.path())
        .output()
        .unwrap();
    assert!(checked.status.success(), "{}", text(&checked));
}

#[test]
#[cfg(unix)]
fn case_teardown_stops_worker_owned_processes_and_resets_native_defaults() {
    let root = app(&[
        (
            "worker.lua",
            r#"
local p=kui.process.spawn('sh',{'-c','sleep 100'})
kui.dispatch {kind='pid',pid=p.pid}
function on_message() end
"#,
        ),
        (
            "main.lua",
            r#"
function init() return {} end
function update(msg) if msg.kind=='worker' then kui.worker('worker.lua') else model.pid=msg.pid end end
function view() return column{} end
"#,
        ),
        (
            "test/main.lua",
            r#"
local t=require('kui.test')
local pid
t.it('owns worker subprocess',function()
  t.expect(kui.platform):to_be('linux')
  t.expect(kui.data_dir:find('/case%-1/data$')~=nil or kui.data_dir:find('case%-1')~=nil):to_be(true)
  t.start {args={'case-one'}};t.expect(kui.args[1]):to_be('case-one');t.expect(t.eval([[return kui.platform]])):to_be('linux');t.dispatch {kind='worker'}
  t.wait_for(function() return t.model().pid~=nil end)
  pid=t.model().pid
  kui.http.configure {base_url='https://previous.test'}
end)
t.it('new case',function()
  local r=kui.process.run('sh',{'-c','kill -0 '..pid..' 2>/dev/null'})
  t.expect(r.ok):to_be(false)
  local ok,_,code=kui.http.try_get('/relative')
  t.expect(ok):to_be(false);t.expect(code):to_be('invalid')
end)
"#,
        ),
    ]);
    let output = run(root.path(), &[]);
    assert!(output.status.success(), "{}", text(&output));
}

fn latest_file(root: &Path) -> PathBuf {
    let output = root.join("test-results");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(output.join("run.json")).unwrap()).unwrap();
    output
        .join(manifest["artifact_root"].as_str().unwrap_or("."))
        .join("file-0000")
}
#[test]
fn reruns_preserve_the_previous_history_and_screenshots() {
    let root = app(&[
        ("main.lua", "function view()return column{text('old')}end"),
        (
            "test/main.lua",
            "local t=require('kui.test');t.it('view',function()t.start()end)",
        ),
    ]);
    let first = run(root.path(), &[]);
    assert!(first.status.success(), "{}", text(&first));
    let old_dir = latest_file(root.path());
    let frame: serde_json::Value =
        serde_json::from_slice(&std::fs::read(old_dir.join("frames/1-1.json")).unwrap()).unwrap();
    let image = old_dir.join(frame["shot"].as_str().unwrap());
    assert!(image.exists());
    std::fs::write(
        root.path().join("main.lua"),
        "function view()return column{text('new')}end",
    )
    .unwrap();
    let second = run(root.path(), &[]);
    assert!(second.status.success(), "{}", text(&second));
    assert!(
        image.exists(),
        "rerun deleted a screenshot that the inspector can still display"
    );
    assert!(
        old_dir.join("frames/1-1.json").exists(),
        "rerun deleted selected history"
    );
}

#[test]
fn agents_can_read_failures_output_and_events_without_a_window() {
    let root = app(&[(
        "test/main.lua",
        r#"
local t=require('kui.test')
t.it('logs',function()print('hello from stdout');kui.log.error('visible logged error')end)
t.it('fails clearly',function()t.expect('actual'):to_be('expected')end)
"#,
    )]);
    let output = run(root.path(), &["--json"]);
    assert!(!output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["status"], "failed");
    assert_eq!(report["ok"], false);
    assert_eq!(report["summary"]["failed"], 1);
    assert_eq!(report["summary"]["passed"], 1);
    assert!(
        report["cases"][1]["error"]
            .as_str()
            .unwrap()
            .contains("expected")
    );
    let query = |verb: &str, args: &[&str]| {
        let mut flags = vec![verb, "test-results", "--json"];
        flags.extend(args);
        run(root.path(), &flags)
    };
    let saved = query("report", &[]);
    assert!(!saved.status.success());
    let parsed: serde_json::Value = serde_json::from_slice(&saved.stdout).unwrap();
    assert_eq!(parsed, report);
    let logs = query("output", &["--file", "file-0000"]);
    assert!(logs.status.success());
    let logs: serde_json::Value = serde_json::from_slice(&logs.stdout).unwrap();
    let log = logs["items"][0]["text"].as_str().unwrap();
    assert!(log.contains("hello from stdout") && log.contains("visible logged error"));
    let events = query("events", &["--case", "logs"]);
    assert!(events.status.success());
    let events: serde_json::Value = serde_json::from_slice(&events.stdout).unwrap();
    assert!(
        events["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["kind"] == "log" && e["data"]["level"] == "error")
    );
    let recorded: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.path().join("test-results/report.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(recorded, report);
}

#[test]
fn empty_or_unmatched_runs_have_machine_readable_errors() {
    let root = app(&[]);
    let empty = run(root.path(), &["--json"]);
    assert!(!empty.status.success());
    let value: serde_json::Value = serde_json::from_slice(&empty.stdout).unwrap();
    assert_eq!(value["status"], "error");
    assert!(value["error"].as_str().unwrap().contains("no tests found"));
    std::fs::create_dir(root.path().join("test")).unwrap();
    std::fs::write(
        root.path().join("test/main.lua"),
        "local t=require('kui.test');t.it('exists',function()end)",
    )
    .unwrap();
    let missing = run(root.path(), &["--json", "--filter", "absent"]);
    assert!(!missing.status.success());
    let value: serde_json::Value = serde_json::from_slice(&missing.stdout).unwrap();
    assert_eq!(value["status"], "error");
    assert_eq!(value["error"], "no test cases matched");
}

#[test]
fn checkout_demo_passes_normally_and_its_opt_in_fault_is_a_recorded_failure() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/lua/test-runner-demo");
    let output = tempfile::tempdir().unwrap();
    let dest = output.path().to_str().unwrap();
    let normal = run(&root, &["--output", dest]);
    assert!(normal.status.success(), "{}", text(&normal));
    assert!(text(&normal).contains("1 passed, 0 failed"));
    let fault = run(
        &root,
        &["fixtures/discount_failure.lua", "--output", dest, "--json"],
    );
    assert!(!fault.status.success());
    let report: serde_json::Value = serde_json::from_slice(&fault.stdout).unwrap();
    assert_eq!(report["status"], "failed");
    assert_eq!(report["summary"]["failed"], 1);
    assert!(
        report["cases"][0]["error"]
            .as_str()
            .unwrap()
            .contains("expected 2000 to be 1600")
    );
    let events = std::fs::read_to_string(report["files"][0]["events"].as_str().unwrap()).unwrap();
    assert!(
        events.contains("discount_percent")
            && events.contains("http")
            && events.contains("Check the discounted total")
    );
    let log = std::fs::read_to_string(report["files"][0]["output_log"].as_str().unwrap()).unwrap();
    assert!(log.contains("20%") && log.contains("$20.00"));
}

#[test]
fn debugger_publishes_the_failure_while_paused_for_inspection() {
    let root = app(&[(
        "test/main.lua",
        "local t=require('kui.test');t.it('inspect this failure',function()t.start();t.expect(1):to_be(2)end)",
    )]);
    let output = root.path().join("artifacts");
    let file = output.join("file-0000");
    std::fs::create_dir_all(file.join("commands")).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let options = serde_json::json!({"root":root.path(),"output":output,"filter":null,"update":false,"system_fonts":false,"headed":false,"debug":true,"ui":true,"every_frame":false,"timeout":3000});
    std::fs::write(output.join("options.json"), options.to_string()).unwrap();
    std::fs::write(output.join("run.json"),serde_json::json!({"version":1,"id":"debug-fixture","status":"running","files":[root.path().join("test/main.lua")]}).to_string()).unwrap();
    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut child = Child(
        Command::new(KUI)
            .arg("__test-file")
            .arg(output.join("options.json"))
            .arg(root.path().join("test/main.lua"))
            .arg(&file)
            .arg(tmp.path())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap(),
    );
    let wait = |f: &dyn Fn() -> bool| {
        let start = std::time::Instant::now();
        while !f() {
            assert!(start.elapsed().as_secs() < 8, "debug checkpoint timed out");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    };
    let paused = || {
        std::fs::read(file.join("status.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
            .is_some_and(|v| v["paused"] == true)
    };
    wait(&|| file.join("frames/1-1.json").exists() && paused());
    std::fs::write(file.join("commands/1.json"), "{\"command\":\"resume\"}").unwrap();
    wait(&|| {
        std::fs::read_to_string(file.join("events.jsonl"))
            .unwrap_or_default()
            .contains("\"kind\":\"assertion\"")
            && paused()
    });
    assert!(
        file.join("results.json").exists(),
        "paused failure is absent from the inspector's results"
    );
    let report = run(root.path(), &["report", output.to_str().unwrap(), "--json"]);
    assert!(!report.status.success());
    let report: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    assert_eq!(report["summary"]["failed"], 1);
    assert!(
        report["cases"][0]["error"]
            .as_str()
            .unwrap()
            .contains("expected 1 to be 2")
    );
    std::fs::write(file.join("commands/2.json"), "{\"command\":\"resume\"}").unwrap();
    wait(&|| {
        std::fs::read(file.join("results.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
            .is_some_and(|v| v["complete"] == true)
    });
    assert!(!child.0.wait().unwrap().success());
    let cases: serde_json::Value =
        serde_json::from_slice(&std::fs::read(file.join("results.json")).unwrap()).unwrap();
    assert_eq!(cases["cases"].as_array().unwrap().len(), 1);
}

#[test]
fn target_recording_survives_removal_and_failure_points_at_the_assertion() {
    let root = app(&[
        (
            "main.lua",
            r#"
function init()return {hidden=false}end
function update(msg)if msg.kind=='hide' then model.hidden=true end end
function view()return column{width='grow',height='grow',not model.hidden and row{role='button',label='Dismiss',width=120,height=40,on_click={kind='hide'},text('Dismiss')} or text('Gone')}end
"#,
        ),
        (
            "test/main.lua",
            "local t=require('kui.test')\nt.it('disappearing target',function()\n t.start()\n t.click('Dismiss')\n t.click(200,150)\n t.expect(t.model().hidden):to_be(false)\nend)\n",
        ),
    ]);
    let out = run(root.path(), &["--json"]);
    assert!(!out.status.success(), "{}", text(&out));
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let diagnostic = &report["files"][0]["diagnostics"][0];
    assert_eq!(
        diagnostic["source"]["file"],
        root.path()
            .join("test/main.lua")
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .as_ref()
    );
    assert_eq!(diagnostic["source"]["line"], 6, "{}", text(&out));
    let events = std::fs::read_to_string(report["files"][0]["events"].as_str().unwrap()).unwrap();
    let targets = events
        .lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
        .filter(|e| e["kind"] == "target")
        .collect::<Vec<_>>();
    assert_eq!(targets.len(), 2, "{events}");
    assert_eq!(targets[0]["data"]["label"], "Dismiss");
    assert_eq!(targets[0]["data"]["rect"]["w"], 120.0);
    assert_eq!(targets[1]["data"]["point"]["x"], 200.0);
}

#[test]
fn a_successful_exit_without_a_complete_result_is_a_file_failure() {
    let root = app(&[(
        "test/main.lua",
        "local t=require('kui.test');t.it('first',function()t.expect(1):to_be(1)end);t.it('aborted',function()os.exit(0)end);t.it('never ran',function()error('must run')end)",
    )]);
    let out = run(root.path(), &["--json"]);
    assert!(!out.status.success(), "{}", text(&out));
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["status"], "failed");
    assert_eq!(report["summary"]["failed"], 1);
    assert!(report["cases"].as_array().unwrap().iter().any(|c| {
        c["error"]
            .as_str()
            .is_some_and(|e| e.contains("incomplete"))
    }));
}

#[test]
#[cfg(unix)]
fn native_resources_used_without_an_app_are_closed_between_cases() {
    let root = app(&[
        ("worker.lua", "while true do end"),
        (
            "test/main.lua",
            r#"
local t=require('kui.test')
local db,child
 t.it('owns native resources',function()
  db=kui.sqlite.open()
  child=kui.process.spawn('sleep',{'60'})
  kui.worker('worker.lua')
 end)
 t.it('previous resources closed',function()
  t.expect(child:kill(0)):to_be(false)
  t.expect(function()db:query('select 1')end):to_throw('closed')
 end)
"#,
        ),
    ]);
    let out = run(root.path(), &["--timeout", "3000"]);
    assert!(out.status.success(), "{}", text(&out));
}

#[test]
fn mock_process_probe_does_not_change_its_exit_or_streams() {
    let root = app(&[(
        "test/main.lua",
        r#"
local t=require('kui.test')
t.it('probe',function()
 t.mock_process{op='spawn',command='mock-tool',response={stdout='hello',code=7},delay=50}
 t.start()
 t.expect(t.eval([[local p=kui.process.spawn('mock-tool');assert(p:kill(0));return p:read_line()]])):to_be('hello')
 t.expect(t.eval([[local p=kui.process.spawn('mock-tool');assert(p:kill(0));return p:wait()]])):to_be(7)
 t.expect(t.eval([[local p=kui.process.spawn('mock-tool');kui.sleep(100);return p:kill(0)]])):to_be(false)
end)
"#,
    )]);
    let out = run(root.path(), &[]);
    assert!(out.status.success(), "{}", text(&out));
}

#[test]
#[cfg(unix)]
fn descendants_stop_before_the_next_case_uses_their_files() {
    let root = app(&[
        (
            "worker.lua",
            "kui.worker('nested.lua');function on_message()end",
        ),
        (
            "nested.lua",
            "while true do kui.fs.write('marker','alive') end",
        ),
        (
            "test/main.lua",
            r#"
local t=require('kui.test')
t.it('nested worker',function()
 kui.worker('worker.lua')
 local start=os.clock()
 while not kui.fs.exists('marker') do assert(os.clock()-start<2,'nested worker did not start') end
end)
t.it('descendant is stopped',function()
 kui.fs.remove('marker')
 os.execute('sleep 0.1')
 t.expect(kui.fs.exists('marker')):to_be(false)
end)
"#,
        ),
    ]);
    let out = run(root.path(), &["--timeout", "5000"]);
    assert!(out.status.success(), "{}", text(&out));
}

#[test]
fn timeout_after_a_case_failure_is_visible_in_file_diagnostics() {
    let root = app(&[(
        "test/main.lua",
        "local t=require('kui.test');t.it('failed',function()t.expect(1):to_be(2)end);t.it('stalled',function()while true do end end)",
    )]);
    let out = run(root.path(), &["--json", "--timeout", "300"]);
    assert!(!out.status.success());
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["summary"]["failed"], 1);
    assert!(
        report["files"][0]["cases"][0]["error"]
            .as_str()
            .unwrap()
            .contains("timed out")
    );
    assert!(
        report["files"][0]["diagnostics"][0]["message"]
            .as_str()
            .unwrap()
            .contains("timed out")
    );
}
