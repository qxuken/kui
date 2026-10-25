//! The `kui` runner end to end (ADR 0064): apps written to a scratch folder
//! and run headless by the built binary, each ending itself with
//! `kui.exit`, their output read back.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

const KUI: &str = env!("CARGO_BIN_EXE_kui");

/// A fresh folder holding `files`.
fn app(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("kui-cli-tests")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for (path, text) in files {
        let p = dir.join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    dir
}

/// Runs `kui <args>` in `dir`, killing it past `secs`.
fn kui(dir: &Path, args: &[&str], secs: u64) -> Output {
    let mut child = Command::new(KUI)
        .args(args)
        .current_dir(dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("run kui");
    let start = Instant::now();
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if start.elapsed() > Duration::from_secs(secs) {
            let _ = child.kill();
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    child.wait_with_output().unwrap()
}

fn out(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

#[test]
fn the_modules_answer_from_a_task() {
    let dir = app(
        "modules",
        &[(
            "main.lua",
            r#"
            print("json", kui.json.encode({ b = 1, a = { 1, 2 } }))
            print("toml", kui.toml.decode("x = 1\n[t]\ny = 'z'").t.y)
            print("yaml", kui.yaml.decode("a: [1, 2]").a[2])
            print("env", kui.env.FROM_DOTENV, kui.env.KEPT)
            kui.fs.write("out/a.txt", "hello")
            print("fs", kui.fs.read("out/a.txt"), kui.fs.exists("nope"))
            kui.spawn(function()
              local t0 = kui.now()
              kui.sleep(40)
              print("slept", kui.now() - t0 >= 35)
              local db = kui.sqlite.open(":memory:")
              db:exec("create table t(x); insert into t values (1); insert into t values (2);")
              db:exec("insert into t values (:v)", { v = 4 })
              print("sql", #db:query("select * from t"), db:first("select sum(x) s from t").s)
              local p = kui.process.run("echo", { "hi" })
              print("proc", p.code, p.stdout:gsub("%s+$", ""))
              local s = kui.http.serve { port = 0 }
              s:get("/api/:name", function(req) return { hello = req.params.name, q = req.query.q } end)
              s:post("/echo", function(req) return { got = req.json.x } end)
              local base = "http://127.0.0.1:" .. s.port
              local r = kui.http.get(base .. "/api/kit?q=1")
              print("get", r.status, r.json.hello, r.json.q)
              local e = kui.http.post(base .. "/echo", { json = { x = 7 } })
              print("post", e.json.got)
              print("404", kui.http.get(base .. "/nope").status)
              kui.exit(3)
            end)
            "#,
        ),
        (".env", "FROM_DOTENV=yes\nKEPT=from-file\n")],
    );
    let o = Command::new(KUI)
        .args(["run", "--headless", "."])
        .current_dir(&dir)
        .env("KEPT", "from-process")
        .output()
        .unwrap();
    let s = out(&o);
    for want in [
        "json\t{\"a\":[1,2],\"b\":1}",
        "toml\tz",
        "yaml\t2",
        "env\tyes\tfrom-process",
        "fs\thello\tfalse",
        "slept\ttrue",
        "sql\t3\t7",
        "proc\t0\thi",
        "get\t200\tkit\t1",
        "post\t7",
        "404\t404",
    ] {
        assert!(s.contains(want), "missing {want:?} in:\n{s}");
    }
    assert_eq!(o.status.code(), Some(3), "kui.exit's code is the process's");
}

#[test]
fn a_waiting_call_outside_a_task_blocks_and_answers() {
    let dir = app(
        "blocking",
        &[(
            "main.lua",
            r#"
            local t0 = kui.now()
            kui.sleep(30)
            print("top-level sleep", kui.now() - t0 >= 25)
            local db = kui.sqlite.open("data.db")
            db:exec("create table if not exists t(x)")
            print("db", db:first("select count(*) n from t").n)
            kui.exit(0)
            "#,
        )],
    );
    let o = kui(&dir, &["run", "--headless"], 20);
    let s = out(&o);
    assert!(s.contains("top-level sleep\ttrue"), "{s}");
    assert!(s.contains("db\t0"), "{s}");
}

#[test]
fn a_worker_hands_back_a_channel_and_teal_runs_as_it_is() {
    let dir = app(
        "worker",
        &[
            (
                "jobs/sum.lua",
                r#"
                function on_message(msg)
                  local xs = {}
                  for i = 1, msg.n do xs[i] = i * 2 end
                  kui.dispatch({ kind = "done", rows = channel { x = xs } })
                end
                "#,
            ),
            (
                "main.tl",
                r#"
                local record Done
                  kind: string
                  rows: kui.Channel
                end
                global function update(m: any)
                  local d = m as Done
                  if d.kind == "done" then
                    print("worker", #d.rows, d.rows:get(9, "x"))
                    kui.exit(0)
                  end
                end
                kui.worker("jobs/sum.lua"):dispatch({ n = 10 })
                "#,
            ),
        ],
    );
    let o = kui(&dir, &["run", "--headless"], 20);
    assert!(out(&o).contains("worker\t10\t20"), "{}", out(&o));
    let c = kui(&dir, &["check"], 60);
    assert!(out(&c).contains("ok: 1 file(s) checked"), "{}", out(&c));
}

#[test]
fn a_worker_that_throws_arrives_as_a_message() {
    let dir = app(
        "worker-error",
        &[
            ("boom.lua", "function on_message() error('kaboom') end\nerror('at load')\n"),
            (
                "main.lua",
                r#"
                kui.worker("boom.lua", function(msg)
                  print("exit", msg.kind, msg.error ~= nil and msg.error:find("at load") ~= nil)
                  kui.exit(0)
                end)
                "#,
            ),
        ],
    );
    let o = kui(&dir, &["run", "--headless"], 20);
    assert!(out(&o).contains("exit\tworker_exit\ttrue"), "{}", out(&o));
}

#[test]
fn check_names_the_mistakes_in_teal() {
    let dir = app(
        "check",
        &[(
            "main.tl",
            r#"local kit = require("kit.ui")
global function view(_env: kui.Env): kui.Node
  kui.sleepz(1)
  local r = kui.http.get("x")
  local s: string = r.status
  return column { kit.button({ label = "x", variantt = "primary" }) }
end
"#,
        )],
    );
    let o = kui(&dir, &["check"], 60);
    let s = out(&o);
    assert_eq!(o.status.code(), Some(1));
    assert!(s.contains("main.tl:3:7: invalid key 'sleepz'"), "{s}");
    assert!(s.contains("main.tl:5:"), "{s}");
    assert!(s.contains("unknown field variantt"), "{s}");
}

#[test]
fn watch_reloads_the_app_and_keeps_its_model() {
    let dir = app(
        "watch",
        &[(
            "main.lua",
            r#"
            model = model or { loads = 0 }
            model.loads = model.loads + 1
            print("load", model.loads)
            if model.loads == 1 then
              kui.spawn(function()
                kui.sleep(400)
                kui.fs.append("main.lua", "\n-- touched\n")
              end)
            else
              kui.exit(0)
            end
            "#,
        )],
    );
    let o = kui(&dir, &["run", "--headless", "--watch"], 20);
    let s = out(&o);
    assert!(s.contains("load\t2"), "the reload ran with the model kept:\n{s}");
    assert_eq!(o.status.code(), Some(0), "{s}");
}

#[test]
fn watch_keeps_running_past_a_broken_save_and_takes_the_fix() {
    let dir = app("broken", &[("main.lua", "this is not lua (\n")]);
    let fixer = {
        let dir = dir.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(900));
            std::fs::write(dir.join("main.lua"), "print('fixed')\nkui.exit(0)\n").unwrap();
        })
    };
    let o = kui(&dir, &["run", "--headless", "--watch"], 20);
    fixer.join().unwrap();
    let s = out(&o);
    assert!(s.contains("fixed"), "{s}");
    assert_eq!(o.status.code(), Some(0), "{s}");
}

#[test]
fn a_broken_app_without_watch_says_why_and_fails() {
    let dir = app("syntax", &[("main.lua", "local x = (\n")]);
    let o = kui(&dir, &["run", "--headless"], 20);
    assert_ne!(o.status.code(), Some(0));
    assert!(out(&o).contains("main.lua"), "{}", out(&o));
}

#[test]
fn an_empty_folder_says_what_it_looked_for() {
    let dir = app("empty", &[("README.md", "nothing")]);
    let o = kui(&dir, &["run", "--headless"], 20);
    let s = out(&o);
    assert!(s.contains("main.lua") && s.contains("kui new"), "{s}");
}

#[test]
fn new_starts_an_app_that_runs() {
    let root = app("new", &[]);
    std::fs::create_dir_all(&root).unwrap();
    let o = kui(&root, &["new", "hello"], 20);
    assert!(o.status.success(), "{}", out(&o));
    let dir = root.join("hello");
    assert!(dir.join("main.lua").exists() && dir.join("kui.config.lua").exists());
    // It draws: a frame with Kit in it, then out.
    std::fs::write(
        dir.join("stop.lua"),
        "kui.timer.after(200, function() kui.exit(0) end)\n",
    )
    .unwrap();
    let main = std::fs::read_to_string(dir.join("main.lua")).unwrap();
    std::fs::write(dir.join("main.lua"), format!("{main}\nrequire('stop')\n")).unwrap();
    let o = kui(&dir, &["run", "--headless"], 20);
    let s = out(&o);
    assert!(o.status.success(), "{s}");
    assert!(!s.contains("warning"), "{s}");
}

/// The Kit app (addons/kit/app), copied with an entry that walks it.
fn kit_app(name: &str, entry: &str) -> PathBuf {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../addons/kit/app");
    let mut files: Vec<(String, String)> = Vec::new();
    for rel in ["main.lua", "pages.lua", "jobs/generate.lua"] {
        files.push((rel.to_string(), std::fs::read_to_string(src.join(rel)).unwrap()));
    }
    files.push(("walk.lua".into(), entry.into()));
    let refs: Vec<(&str, &str)> = files.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    app(name, &refs)
}

#[test]
fn the_kit_app_draws_every_page_in_every_skin_without_a_warning() {
    let dir = kit_app(
        "kit-app",
        r#"
        require("main")
        model = init()
        local steps = {}
        for _, p in ipairs(PAGES) do steps[#steps + 1] = { kind = "page", change = { value = p.value } } end
        for _, s in ipairs({ "phosphor", "neutral", "ledger" }) do steps[#steps + 1] = { kind = "skin", change = { value = s } } end
        steps[#steps + 1] = { kind = "density", toggle = { on = true } }
        steps[#steps + 1] = { kind = "shadows", toggle = { on = false } }
        steps[#steps + 1] = { kind = "toast_all" }
        steps[#steps + 1] = { kind = "menu", open = true }
        steps[#steps + 1] = { kind = "dialog", open = true }
        local i = 0
        kui.timer.every(120, function()
          i = i + 1
          if steps[i] then kui.dispatch(steps[i]) else
            print("rows", model.rows and #model.rows or 0, "spans", model.spans and #model.spans or 0)
            kui.exit(0)
          end
        end)
        "#,
    );
    let o = kui(&dir, &["run", "--headless", "walk.lua"], 60);
    let s = out(&o);
    assert!(o.status.success(), "{s}");
    assert!(!s.contains("warning"), "{s}");
    assert!(s.contains("rows\t100000\tspans\t18000"), "the worker's channels arrived:\n{s}");
}

/// The Kit app is Kit's gallery: every component its schema describes is
/// placed on one of its pages (by its wrapper, `kit.<name>(`), so a new
/// component cannot ship without a live demo.
#[test]
fn the_kit_app_places_every_component() {
    let kit = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../addons/kit");
    let schema = std::fs::read_to_string(kit.join("schema.lua")).unwrap();
    let app = ["main.lua", "pages.lua"]
        .iter()
        .map(|f| std::fs::read_to_string(kit.join("app").join(f)).unwrap())
        .collect::<String>();
    // A component's entry opens a line of its own: `    { "name",`.
    let names: Vec<&str> = schema
        .lines()
        .filter_map(|l| l.strip_prefix("    { \""))
        .filter_map(|l| l.split('"').next())
        .collect();
    assert!(names.len() >= 39, "the schema's components: {names:?}");
    let missing: Vec<&&str> = names.iter().filter(|n| !app.contains(&format!("kit.{n}("))).collect();
    assert!(missing.is_empty(), "components with no demo in the Kit app: {missing:?}");
    // And it runs as an app of its own: a name, an id, a window.
    let config = std::fs::read_to_string(kit.join("app/kui.config.lua")).unwrap();
    assert!(config.contains("name = \"Kit\"") && config.contains("id = \"dev.kui.kit.gallery\""), "{config}");
}

/// Runs a Kit sample's own headless test (addons/kit/samples/<name>/test),
/// in place: each writes only to the temp folder and serves on a free port.
fn kit_sample(name: &str) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../addons/kit/samples").join(name);
    let o = kui(&dir, &["run", "test", "--headless"], 60);
    let s = out(&o);
    assert!(o.status.success() && s.contains("PASS: 0 failure(s)"), "{name}:\n{s}");
    assert!(!s.contains("warning"), "{name}:\n{s}");
}

/// The book's runner chapter (docs/book/src/runner.md) ends with this app
/// and its test, so the chapter cannot drift from what runs.
#[test]
fn the_books_task_list_passes_its_own_test() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/lua/tutorial/tasks");
    let o = kui(&dir, &["run", "test", "--headless"], 60);
    let s = out(&o);
    assert!(o.status.success() && s.contains("PASS: 0 failure(s)"), "{s}");
    assert!(!s.contains("warning"), "{s}");
}

#[test]
fn the_relay_sample_sends_real_requests_to_its_mock_api() {
    kit_sample("relay");
}

#[test]
fn the_watchtower_sample_runs_its_teal_dashboard() {
    kit_sample("watchtower");
}

#[test]
fn the_field_notes_sample_keeps_its_notes_in_sqlite() {
    kit_sample("field-notes");
}

#[test]
fn the_preferences_sample_validates_saves_and_reads_back() {
    kit_sample("preferences");
}

#[test]
fn the_watchtower_sample_type_checks() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../addons/kit/samples/watchtower");
    let o = kui(&dir, &["check", "."], 60);
    let s = out(&o);
    assert!(o.status.success() && s.contains("ok:"), "{s}");
}

#[test]
fn types_writes_what_editors_read() {
    let dir = app("types", &[("main.lua", "")]);
    let o = kui(&dir, &["types"], 20);
    assert!(o.status.success(), "{}", out(&o));
    for f in [".kui/types/kui.d.tl", ".kui/types/kit/ui.d.tl", ".kui/types/kui.lua", ".kui/types/kui_runner.lua", ".kui/types/kit/ui.lua", ".luarc.json", "tlconfig.lua"] {
        assert!(dir.join(f).exists(), "{f}");
    }
    let decls = std::fs::read_to_string(dir.join(".kui/types/kui.d.tl")).unwrap();
    assert!(decls.contains("serve: function(opts?: ServeOpts): Server"), "the runner's API is in the kui record");
}

#[test]
fn the_app_reads_its_arguments_folder_and_platform() {
    let dir = app(
        "args",
        &[(
            "main.lua",
            r#"
            print("args", table.concat(kui.args, ","))
            print("cwd", kui.cwd == kui.dir)
            print("platform", kui.platform ~= nil and kui.platform ~= "", kui.cpus >= 1, kui.process.pid > 0)
            print("create", kui.fs.create("lock", "a"), kui.fs.create("lock", "b"), kui.fs.read("lock"))
            kui.exit(0)
            "#,
        )],
    );
    let o = kui(&dir, &["run", "--headless", "--", "--port", "3001", "x y"], 20);
    let s = out(&o);
    assert!(s.contains("args\t--port,3001,x y"), "{s}");
    assert!(s.contains("cwd\ttrue"), "{s}");
    assert!(s.contains("platform\ttrue\ttrue\ttrue"), "{s}");
    assert!(s.contains("create\ttrue\tfalse\ta"), "a second create leaves the first's file:\n{s}");
}

#[cfg(unix)]
#[test]
fn a_group_dies_whole_and_spawned_processes_end_with_the_app() {
    let dir = app(
        "groups",
        &[
            (
                "tree.sh",
                "#!/bin/sh\n# a parent that starts a child and waits on it, as npx does\nsleep 31337 &\necho started\nwait\n",
            ),
            (
                "main.lua",
                r#"
                local function sleepers()
                  local r = kui.process.run("pgrep", { "-f", "sleep 3133" })
                  local n = 0
                  for _ in r.stdout:gmatch("%d+") do n = n + 1 end
                  return n
                end
                kui.spawn(function()
                  local p = kui.process.spawn("sh", { "tree.sh" }, { group = true })
                  print("line", p:read_line())
                  print("alive", kui.process.kill(p.pid, 0))
                  print("term", p:kill("term"))
                  print("code", p:wait())
                  kui.sleep(100)
                  print("orphans", sleepers())
                  print("after", p:kill())
                  -- what is still running when the app ends goes with it
                  kui.process.spawn("sh", { "-c", "sleep 31338" })
                  kui.exit(0)
                end)
                "#,
            ),
        ],
    );
    let o = kui(&dir, &["run", "--headless"], 20);
    let s = out(&o);
    assert!(s.contains("line\tstarted"), "{s}");
    assert!(s.contains("alive\ttrue"), "{s}");
    assert!(s.contains("term\ttrue"), "{s}");
    assert!(s.contains("code\tnil"), "a signal's exit has no code:\n{s}");
    assert!(s.contains("orphans\t0"), "the group's child died with it:\n{s}");
    assert!(s.contains("after\tfalse"), "an exited process is not signalled again:\n{s}");
    std::thread::sleep(Duration::from_millis(200));
    let left = Command::new("pgrep").args(["-f", "sleep 31338"]).output().unwrap();
    assert!(left.stdout.is_empty(), "the app's process ended with it");
}

#[test]
fn next_frame_hands_update_the_env_and_on_exit_runs_last() {
    let dir = app(
        "frame",
        &[(
            "main.lua",
            r#"
            function update(msg)
              if msg.kind == "go" then
                kui.next_frame(function(env)
                  print("env", type(env.set_clipboard), env.viewport_w > 0)
                  kui.exit(0)
                end)
              end
            end
            function on_exit() print("on_exit") end
            function view(env) return column {} end
            kui.dispatch({ kind = "go" })
            "#,
        )],
    );
    let o = kui(&dir, &["run", "--headless"], 20);
    let s = out(&o);
    assert!(s.contains("env\tfunction\ttrue"), "{s}");
    assert!(s.contains("on_exit"), "{s}");
}

#[test]
fn a_manifest_sets_the_window() {
    let dir = app(
        "manifest",
        &[
            (
                "kui.config.lua",
                r#"return { name = "Win", min_size = { w = 400, h = 300 }, chrome = "custom", titlebar = "medium" }"#,
            ),
            ("main.lua", "kui.exit(0)"),
        ],
    );
    let o = kui(&dir, &["run", "--headless"], 20);
    assert!(o.status.success(), "{}", out(&o));
    let bad = app("bad-manifest", &[("kui.config.lua", r#"return { chrome = "glass" }"#), ("main.lua", "")]);
    let o = kui(&bad, &["run", "--headless"], 20);
    assert!(out(&o).contains("native, custom or borderless"), "{}", out(&o));
}

#[test]
fn parse_args_reads_the_line_and_says_what_went_wrong_before_any_window() {
    let main = r#"
        local o = kui.parse_args {
          about = "a parser test",
          options = {
            port = { type = "integer", default = 3000, short = "p", help = "the port" },
            host = { default = "127.0.0.1", env = "PARSE_HOST" },
            dry_run = { type = "boolean" },
            tag = { many = true },
            mode = { choices = { "fast", "slow" }, default = "fast" },
          },
          args = { { name = "files", many = true } },
        }
        print("got", o.port, o.host, o.dry_run, kui.json.encode(o.tag), o.mode, kui.json.encode(o.files))
        function view(env) return column {} end
        kui.exit(0)
    "#;
    let dir = app("parse-args", &[("main.lua", main)]);
    let o = kui(&dir, &["run", "--", "-p", "8080", "--dry-run", "--tag", "a", "--tag=b", "--mode=slow", "x", "y"], 20);
    let s = out(&o);
    assert!(s.contains(r#"got	8080	127.0.0.1	true	["a","b"]	slow	["x","y"]"#), "{s}");
    let o = kui(&dir, &["run"], 20);
    assert!(out(&o).contains(r#"got	3000	127.0.0.1	false	[]	fast	[]"#), "{}", out(&o));

    // Help and mistakes end the app before its window, with no error of the runner's.
    let o = kui(&dir, &["run", "--", "--help"], 20);
    let s = out(&o);
    assert!(o.status.success(), "{s}");
    assert!(s.contains("parse-args") && s.contains("a parser test") && s.contains("-p, --port <integer>"), "{s}");
    assert!(!s.contains("kui:") && !s.contains("got"), "{s}");
    let o = kui(&dir, &["run", "--", "--prot", "1"], 20);
    let s = out(&o);
    assert_eq!(o.status.code(), Some(2), "{s}");
    assert!(s.contains("unknown option --prot (did you mean --port?)"), "{s}");
    assert!(!s.contains("stack traceback"), "{s}");
    let o = kui(&dir, &["run", "--", "--mode", "medium"], 20);
    assert!(out(&o).contains("expected one of fast, slow"), "{}", out(&o));
}

#[test]
fn hashes_arrays_walks_and_logs() {
    let dir = app(
        "batteries",
        &[
            ("src/a.txt", "alpha"),
            ("src/deep/b.txt", "beta"),
            ("node_modules/x/c.txt", "skipped"),
            (
                "main.lua",
                r#"
                print("sha256", kui.hash.sha256("abc"))
                print("sha1", kui.hash.sha1("abc"))
                print("fnv", kui.hash.fnv("abc"))
                print("array", kui.json.encode({ empty = kui.json.array(), map = {} }))
                local back = kui.json.decode('{"list":[],"obj":{}}')
                print("round", kui.json.encode(back))
                kui.log.warn("careful", 1)
                kui.spawn(function()
                  local files = kui.fs.walk(".", { match = "*.txt" })
                  print("walk", table.concat(files, ","))
                  local texts = kui.fs.read_all({ "src/a.txt", "nope.txt", "src/deep/b.txt" })
                  print("read_all", texts[1], texts[2], texts[3])
                  local cmd = kui.process.command_line(kui.process.pid)
                  print("cmd", cmd ~= nil and cmd:find("kui") ~= nil, kui.process.command_line(999999) == nil)
                  kui.exit(0)
                end)
                "#,
            ),
        ],
    );
    let o = kui(&dir, &["run", "--headless"], 20);
    let s = out(&o);
    assert!(s.contains("sha256\tba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"), "{s}");
    assert!(s.contains("sha1\ta9993e364706816aba3e25717850c26c9cd0d89d"), "{s}");
    assert!(s.contains("fnv\te71fa2190541574b"), "{s}");
    assert!(s.contains(r#"array	{"empty":[],"map":{}}"#), "{s}");
    assert!(s.contains(r#"round	{"list":[],"obj":{}}"#), "an empty list decoded stays a list:\n{s}");
    assert!(s.contains("[warn] careful\t1"), "{s}");
    assert!(s.contains("walk\tsrc/a.txt,src/deep/b.txt"), "node_modules is skipped:\n{s}");
    assert!(s.contains("read_all\talpha\tfalse\tbeta"), "{s}");
    assert!(s.contains("cmd\ttrue\ttrue"), "{s}");
}

#[test]
fn the_devtools_log_tab_shows_what_the_app_printed() {
    let dir = app(
        "log-tab",
        &[(
            "main.lua",
            r#"
            print("hello from print")
            kui.log.error("a bad thing")
            kui.devtools("log")
            function view(env) return column { text("app") } end
            "#,
        )],
    );
    let shot = dir.join("shot.png");
    let o = kui(&dir, &["run", "--shot", shot.to_str().unwrap(), "--after", "300"], 30);
    assert!(o.status.success(), "{}", out(&o));
    assert!(shot.exists(), "{}", out(&o));
}

// -- kui pack (ADR 0068) -----------------------------------------------------

/// Runs `kui <args>` in `dir` with the runner's cache in `cache` and the
/// extra environment `env`, killing it past `secs`.
fn kui_env(dir: &Path, args: &[&str], env: &[(&str, &str)], cache: &Path, secs: u64) -> Output {
    let mut cmd = Command::new(KUI);
    cmd.args(args)
        .current_dir(dir)
        .env("KUI_CACHE_DIR", cache)
        .env_remove("KUI_FLAGS")
        .env_remove("KUI_LOG")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    for (k, v) in env {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().expect("run kui");
    let start = Instant::now();
    while child.try_wait().unwrap().is_none() {
        if start.elapsed() > Duration::from_secs(secs) {
            let _ = child.kill();
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    child.wait_with_output().unwrap()
}

/// An app that says what it can see of itself when packed, then ends.
fn packer(name: &str) -> PathBuf {
    app(
        name,
        &[
            (
                "kui.config.lua",
                r#"return { name = "Packer", id = "dev.kui.test.packer", version = "2.0.0" }"#,
            ),
            ("data.txt", "an asset"),
            ("lib.lua", r#"return { said = "a module" }"#),
            ("test/skip.lua", "error('test/ is not packed')"),
            (
                "main.lua",
                r#"
                local p = kui.packed
                print("packed", p ~= nil, p and p.bytecode)
                print("args", table.concat(kui.args, ","))
                print("id", kui.id, kui.app_version)
                print("asset", kui.fs.read("data.txt"))
                print("module", require("lib").said)
                print("test dir", kui.fs.exists("test/skip.lua"))
                local ok, err = pcall(kui.fs.write, "x.txt", "no")
                print("write", ok, (tostring(err):find("read%-only")) ~= nil)
                kui.exit(0)
                "#,
            ),
        ],
    )
}

#[test]
fn a_packed_archive_runs_from_itself_read_only_with_its_arguments() {
    let dir = packer("pack-kui");
    let cache = dir.join(".cache");
    let o = kui_env(&dir, &["pack", "--format", "kui", "--out", "dist"], &[], &cache, 60);
    assert!(o.status.success(), "{}", out(&o));
    let archive = dir.join("dist/packer-2.0.0.kui");
    assert!(archive.is_file(), "{}", out(&o));
    let o = kui_env(&dir, &["run", archive.to_str().unwrap(), "--headless", "--", "a", "b"], &[], &cache, 60);
    let s = out(&o);
    assert!(o.status.success(), "{s}");
    assert!(s.contains("packed\ttrue\tfalse"), "{s}");
    assert!(s.contains("args\ta,b"), "{s}");
    assert!(s.contains("id\tdev.kui.test.packer\t2.0.0"), "{s}");
    assert!(s.contains("asset\tan asset"), "{s}");
    assert!(s.contains("module\ta module"), "{s}");
    assert!(s.contains("test dir\tfalse"), "{s}");
    assert!(s.contains("write\tfalse\ttrue"), "a packed app's own files are read-only:\n{s}");
}

#[test]
fn a_bytecode_archive_runs_and_bytecode_loads_nowhere_else() {
    let dir = packer("pack-bytecode");
    let cache = dir.join(".cache");
    let o = kui_env(&dir, &["pack", "--format", "kui", "--bytecode", "--out", "dist"], &[], &cache, 60);
    assert!(o.status.success(), "{}", out(&o));
    let archive = dir.join("dist/packer-2.0.0.kui");
    let o = kui_env(&dir, &["run", archive.to_str().unwrap(), "--headless"], &[], &cache, 60);
    let s = out(&o);
    assert!(s.contains("packed\ttrue\ttrue") && s.contains("module\ta module"), "{s}");

    let plain = app(
        "bytecode-refused",
        &[(
            "main.lua",
            r#"
            local f, err = load(string.dump(function() return 1 end))
            print("load", f == nil, err)
            kui.exit(0)
            "#,
        )],
    );
    let o = kui_env(&plain, &["run", "--headless"], &[], &cache, 60);
    let s = out(&o);
    assert!(s.contains("load\ttrue") && s.contains("binary chunk"), "{s}");
}

/// A runner with an archive appended runs it: every argument the app's,
/// the runner's own from KUI_FLAGS. Packed for Linux with this machine's
/// own runner, so it runs here.
#[cfg(unix)]
#[test]
fn a_fused_binary_runs_its_app_with_every_argument_the_apps() {
    let dir = packer("pack-fused");
    let cache = dir.join(".cache");
    let o = kui_env(
        &dir,
        &["pack", "--target", "linux-x64", "--format", "bin", "--runner", KUI, "--out", "dist"],
        &[],
        &cache,
        60,
    );
    assert!(o.status.success(), "{}", out(&o));
    let tar = Command::new("tar")
        .args(["-xzf", "packer-2.0.0-linux-x64.tar.gz"])
        .current_dir(dir.join("dist"))
        .status()
        .unwrap();
    assert!(tar.success());
    let mut child = Command::new(dir.join("dist/packer"))
        .args(["--port", "3001"])
        .env("KUI_FLAGS", "--headless")
        .env("KUI_CACHE_DIR", &cache)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let start = Instant::now();
    while child.try_wait().unwrap().is_none() && start.elapsed() < Duration::from_secs(60) {
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = child.kill();
    let o = child.wait_with_output().unwrap();
    let s = out(&o);
    assert!(s.contains("args\t--port,3001"), "{s}");
    assert!(s.contains("asset\tan asset"), "{s}");
}

#[test]
fn pack_asks_for_an_id_and_run_warns_on_a_pin_it_does_not_cover() {
    let dir = app(
        "pack-no-id",
        &[
            ("kui.config.lua", r#"return { name = "NoId", kui = "0.0.1" }"#),
            ("main.lua", "print('ran') kui.exit(0)"),
        ],
    );
    let cache = dir.join(".cache");
    let o = kui_env(&dir, &["pack", "--format", "kui"], &[], &cache, 60);
    assert!(!o.status.success() && out(&o).contains("needs an id"), "{}", out(&o));
    let o = kui_env(&dir, &["run", "--headless"], &[], &cache, 60);
    let s = out(&o);
    assert!(s.contains("ran") && s.contains("pins kui 0.0.1"), "{s}");
}
