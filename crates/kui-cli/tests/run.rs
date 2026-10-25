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
