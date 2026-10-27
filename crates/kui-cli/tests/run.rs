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

/// Runs `kui <args>` in `dir`, killing it past `secs` (times
/// `KUI_TEST_TIMEOUT_SCALE`, for a slow machine) and saying so.
///
/// Its output is read while it runs, a thread per pipe: a pipe nobody
/// reads holds about 4 KB on Windows (64 on the unixes), and a run that
/// wrote more blocked on its own write until it was killed here, its
/// output lost — three tests failed on the first Windows runner that way,
/// reading as apps that said nothing.
fn kui(dir: &Path, args: &[&str], secs: u64) -> Output {
    use std::io::Read;
    let scale: f64 = std::env::var("KUI_TEST_TIMEOUT_SCALE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1.0);
    let secs = (secs as f64 * scale).ceil() as u64;
    let mut child = Command::new(KUI)
        .args(args)
        .current_dir(dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("run kui");
    fn drain(mut r: impl Read + Send + 'static) -> std::thread::JoinHandle<Vec<u8>> {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = r.read_to_end(&mut buf);
            buf
        })
    }
    let stdout = drain(child.stdout.take().unwrap());
    let stderr = drain(child.stderr.take().unwrap());
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if start.elapsed() > Duration::from_secs(secs) {
            let _ = child.kill();
            eprintln!(
                "kui {} in {}: killed after {secs} s",
                args.join(" "),
                dir.display()
            );
            break child.wait().unwrap();
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    Output {
        status,
        stdout: stdout.join().unwrap(),
        stderr: stderr.join().unwrap(),
    }
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
        &[
            (
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
            (".env", "FROM_DOTENV=yes\nKEPT=from-file\n"),
        ],
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
fn a_try_twin_returns_false_and_the_message_where_its_call_raises() {
    let dir = app(
        "try-twins",
        &[
            ("bad.json", "{ not json"),
            (
                "typed.tl",
                r#"
                local ok, got = kui.http.try_get("http://127.0.0.1:9/")
                if got is kui.Response then print(got.status) else print(ok, got) end
                local _, lines = kui.fs.try_lines("x.txt")
                if lines is {string} then print(#lines) end
                "#,
            ),
            (
                "main.lua",
                r#"
                print("decode", kui.json.try_decode('{"a":[1]}'))
                local ok, err = kui.json.try_decode(kui.fs.read("bad.json"))
                print("bad json", ok, type(err), err)
                print("null", kui.json.try_decode("null"))
                print("read", kui.fs.try_read("missing.txt"))
                print("create", kui.fs.try_create("made.txt", "x"), kui.fs.try_create("made.txt", "x"))
                print("worker", kui.try_worker("nope.lua"))
                local db_ok, db = kui.sqlite.try_open(":memory:")
                print("db", db_ok, db:try_query("select * from nowhere"))
                print("tx", db:try_transaction(function(d) d:exec("create table t(x)"); error("undo") end))
                print("rolled back", db:try_first("select * from t"))
                print("no url", kui.http.try_get(""))
                print("wrong type raises", (pcall(kui.fs.try_read)))
                local _, plain = pcall(function() local v = kui.json.decode("{"); return v end)
                print("plain", plain)
                kui.spawn(function()
                  print("run", kui.process.try_run("kui-no-such-command"))
                  local hit, res = kui.http.try_get("http://127.0.0.1:9/")
                  print("refused", hit, type(res))
                  print("walk", kui.fs.try_walk("no-such-dir"))
                  kui.exit(0)
                end)
                "#,
            ),
        ],
    );
    let o = kui(&dir, &["run", "--headless"], 20);
    let s = out(&o);
    assert!(s.contains("decode\ttrue\ttable: "), "{s}");
    assert!(s.contains("bad json\tfalse\tstring\tjson: "), "{s}");
    assert!(
        s.contains("null\ttrue\tnil"),
        "a decoded null is a value, not a failure:\n{s}"
    );
    assert!(
        s.contains("missing.txt: ") && !s.contains("runtime error"),
        "the message alone:\n{s}"
    );
    assert!(s.contains("create\ttrue\ttrue\tfalse"), "{s}");
    assert!(s.contains("worker\tfalse\tkui.worker: no file"), "{s}");
    assert!(s.contains("db\ttrue\tfalse\tno such table: nowhere"), "{s}");
    assert!(s.contains("tx\tfalse\t") && s.contains("undo"), "{s}");
    assert!(s.contains("rolled back\tfalse\tno such table: t"), "{s}");
    assert!(s.contains("run\tfalse\tkui-no-such-command: "), "{s}");
    assert!(s.contains("refused\tfalse\tstring"), "{s}");
    assert!(s.contains("walk\tfalse\t"), "{s}");
    assert!(
        s.contains("no url\tfalse\thttp: no url"),
        "wrong input is a failure, not a raise:\n{s}"
    );
    assert!(s.contains("wrong type raises\tfalse"), "{s}");
    assert!(
        s.contains("plain\t") && s.contains("main.lua:15: json: "),
        "the plain call raises at its caller's line:\n{s}"
    );
    assert!(!s.contains("traceback"), "{s}");
    let c = kui(&dir, &["check"], 60);
    assert!(out(&c).contains("ok: 1 file(s) checked"), "{}", out(&c));
}

#[test]
fn a_worker_that_throws_arrives_as_a_message() {
    let dir = app(
        "worker-error",
        &[
            (
                "boom.lua",
                "function on_message() error('kaboom') end\nerror('at load')\n",
            ),
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
    assert!(
        s.contains("load\t2"),
        "the reload ran with the model kept:\n{s}"
    );
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

/// A view's font file is read under the runner's folder rules (backlog
/// C117): against the app's folder, from wherever `kui run` was started;
/// and the platform reads as the build's (C116).
#[test]
fn a_font_file_is_read_against_the_app_folder() {
    let dir = app(
        "fonts",
        &[
            (
                "main.lua",
                r#"
                function view(env)
                  if not said then
                    said = true
                    print("missing", env.load_font_file("fonts/none.ttf"))
                    print("garbage", env.load_font_file("fonts/garbage.ttf"))
                    print("listed", type(env.system_font_families()))
                    print("platform", env.platform)
                    kui.exit(0)
                  end
                  return column {}
                end
                "#,
            ),
            ("fonts/garbage.ttf", "not a font"),
        ],
    );
    // Started from the folder above, naming the app's.
    let name = dir.file_name().unwrap().to_str().unwrap().to_string();
    let o = kui(dir.parent().unwrap(), &["run", "--headless", &name], 20);
    let s = out(&o);
    assert!(o.status.success(), "{s}");
    let missing = s.lines().find(|l| l.starts_with("missing")).unwrap_or("");
    assert!(
        missing.starts_with("missing\tnil\t")
            && missing.contains(&name)
            && missing.contains("none.ttf"),
        "nil, and the path in the app's folder: {s}"
    );
    let garbage = s.lines().find(|l| l.starts_with("garbage")).unwrap_or("");
    assert!(
        garbage.starts_with("garbage\tnil\t") && garbage.contains("no face"),
        "the file in the app's folder was read, and holds no font: {s}"
    );
    assert!(s.contains("listed\ttable"), "{s}");
    let platform = if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(windows) {
        "windows"
    } else {
        "linux"
    };
    assert!(s.contains(&format!("platform\t{platform}")), "{s}");
}

/// The Lua files under `dir`, as paths relative to it.
fn lua_files(dir: &Path, rel: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir.join(rel)).unwrap() {
        let e = e.unwrap();
        let r = rel.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            lua_files(dir, &r, out);
        } else if r.extension().is_some_and(|x| x == "lua") {
            out.push(r);
        }
    }
}

/// The files under `dir` a test copies of an app: its Lua and its docs
/// (the design system's Markdown and tokens.json), as paths relative to it.
fn app_files(dir: &Path, rel: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir.join(rel)).unwrap() {
        let e = e.unwrap();
        let r = rel.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            app_files(dir, &r, out);
        } else if r
            .extension()
            .is_some_and(|x| x == "lua" || x == "md" || x == "json")
        {
            out.push(r);
        }
    }
}

/// The Kit app (addons/kit/app), every Lua file and doc of it copied but
/// its manifest (whose entry would win over the walker), with an entry
/// that walks it.
fn kit_app(name: &str, entry: &str) -> PathBuf {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../addons/kit/app");
    let mut rels = Vec::new();
    app_files(&src, Path::new(""), &mut rels);
    rels.retain(|r| r != Path::new("kui.config.lua"));
    let mut files: Vec<(String, String)> = Vec::new();
    for rel in rels {
        files.push((
            rel.to_string_lossy().replace('\\', "/"),
            std::fs::read_to_string(src.join(&rel)).unwrap(),
        ));
    }
    files.push(("walk.lua".into(), entry.into()));
    let refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    app(name, &refs)
}

#[test]
fn the_kit_app_draws_every_page_in_every_skin_without_a_warning() {
    // Every card's Source open (each reads its file from the app's folder)
    // and the Theme panel open, every page, then every skin, both held
    // appearances, compact, no shadows, an accent, a radius and a colour.
    let dir = kit_app(
        "kit-app",
        r##"
        require("main")
        model = init()
        local pages = require("pages")
        local steps = { { kind = "theme_open" }, { kind = "theme_colors", toggle = { collapsed = false } } }
        for name in pairs(pages.DEMOS) do steps[#steps + 1] = { kind = "source", name = name } end
        for _, p in ipairs(PAGES) do steps[#steps + 1] = { kind = "page", change = { value = p.value } } end
        -- Back to the Loading page: every shader the shader box shows.
        steps[#steps + 1] = { kind = "page", change = { value = "loading" } }
        for _, s in ipairs({ "shimmer", "rings", "noise" }) do steps[#steps + 1] = { kind = "shader", change = { value = s } } end
        for _, s in ipairs({ "phosphor", "neutral", "ledger" }) do
          steps[#steps + 1] = { kind = "theme", field = "skin", change = { value = s } }
        end
        for _, a in ipairs({ "light", "dark" }) do
          steps[#steps + 1] = { kind = "theme", field = "appearance", change = { value = a } }
        end
        steps[#steps + 1] = { kind = "theme", field = "density", change = { value = "compact" } }
        steps[#steps + 1] = { kind = "theme", field = "shadows", toggle = { on = false } }
        steps[#steps + 1] = { kind = "theme_accent", color = 0x16a34aff }
        steps[#steps + 1] = { kind = "theme_number", field = "radius", change = { value = 6 } }
        steps[#steps + 1] = { kind = "theme_color", role = "panel", half = "dark", change = { text = "#202830" } }
        steps[#steps + 1] = { kind = "theme", field = "font_ui", change = { value = "serif" } }
        -- The family list: a pick for the data face, a filter that leaves
        -- nothing, and a font file that is not there.
        steps[#steps + 1] = { kind = "theme", field = "font_target", change = { value = "data" } }
        steps[#steps + 1] = { kind = "theme_font_pick", select = { row = 0 } }
        steps[#steps + 1] = { kind = "theme_font_filter", change = { text = "kui-no-such-family" } }
        steps[#steps + 1] = { kind = "theme_font_file", change = { text = "fonts/none.ttf" } }
        steps[#steps + 1] = { kind = "toast_all" }
        steps[#steps + 1] = { kind = "menu", open = true }
        steps[#steps + 1] = { kind = "dialog", open = true }
        local i = 0
        kui.timer.every(120, function()
          i = i + 1
          if steps[i] then kui.dispatch(steps[i]) else
            print("rows", model.rows and #model.rows or 0, "spans", model.spans and #model.spans or 0)
            print("profile", model.profile)
            for _, card in ipairs({ "charts/live_line", "charts/bars", "charts/histogram", "data/log" }) do
              print("source", card, table.concat(pages.files_of(card), " "))
            end
            print(require("theme").lua(model.theme))
            kui.exit(0)
          end
        end)
        "##,
    );
    let o = kui(&dir, &["run", "--headless", "walk.lua"], 90);
    let s = out(&o);
    assert!(o.status.success(), "{s}");
    assert!(!s.contains("warning"), "{s}");
    assert!(
        !s.contains("could not be read"),
        "every card's source was read:\n{s}"
    );
    assert!(
        s.contains("rows\t100000\tspans\t18000"),
        "the worker's channels arrived:\n{s}"
    );
    assert!(
        s.contains("profile\tloaded"),
        "the loading card loaded:\n{s}"
    );
    // A chart card's Source shows where its data is made.
    for (card, data) in [
        ("charts/live_line", "demos/feeds.lua"),
        ("charts/bars", "jobs/generate.lua"),
        ("charts/histogram", "jobs/generate.lua"),
        ("data/log", "demos/feeds.lua"),
    ] {
        let line = s
            .lines()
            .find(|l| l.starts_with(&format!("source\t{card}\t")))
            .unwrap_or_else(|| panic!("no source line for {card}:\n{s}"));
        assert!(line.contains(data), "{card}'s Source shows {data}: {line}");
    }
    for line in [
        "appearance = \"dark\"",
        "radius = 6,",
        "accent = 0x16a34aff,",
        "panel = { dark = 0x202830ff },",
        "font_ui = { \"serif\" },",
        "font_data = { \"",
    ] {
        assert!(s.contains(line), "the theme as Lua has {line}:\n{s}");
    }
}

/// The walk the paint audit runs over (ADR 0074): every page in every skin,
/// each scrolled from its top to its end under the toolbar, then what
/// moves while it is drawn — the live line chart sliding under the edge of
/// its scroller, a slider springing to its ends, a checkbox's ring swept
/// past the edge, skeletons shimmering, toasts leaving, a menu and a dialog
/// open — each step a frame or more.
const KIT_AUDIT_WALK: &str = r##"
local kit = require("kit.ui")
local checkbox = kit.checkbox
kit.checkbox = function(p)
  if p.label ~= "Follow the log" then return checkbox(p) end
  return column { key = "probe.box", on_layout = { kind = "noop" }, checkbox(p) }
end
require("main")
model = init()
local inner = view
local scroll_y, reveal, box, pressed, ringed = nil, false, nil, nil, false
function view(env)
  local t = inner(env)
  if scroll_y then env.set_scroll("page." .. model.page, 0, scroll_y); scroll_y = nil end
  if reveal then env.reveal("probe.box"); reveal = false end
  if model.page == "inputs" then box = env.layout_of("probe.box") end
  if pressed == true then pressed = env.focus end
  ringed = pressed ~= nil and pressed ~= true and env.focus == pressed
  return t
end
local steps = {}
local function add(f) steps[#steps + 1] = f end
local function send(m) add(function() kui.dispatch(m) end) end
local function scroll(y) add(function() scroll_y = y; kui.dispatch({ kind = "audit_scroll" }) end) end
local function wait(n) for _ = 1, n do add(function() end) end end
local function page(p) send({ kind = "page", change = { value = p } }) end
for _, s in ipairs({ { "phosphor", "dark" }, { "neutral", "light" }, { "ledger", "dark" } }) do
  send({ kind = "theme", field = "skin", change = { value = s[1] } })
  send({ kind = "theme", field = "appearance", change = { value = s[2] } })
  for _, p in ipairs(PAGES) do
    page(p.value)
    for _, y in ipairs({ 0, 90, 260, 700, 1e9 }) do scroll(y) end
  end
  -- The live chart sliding under the scroller's top edge: it is the
  -- Charts page's first card, swept past the edge a few px a frame.
  page("charts")
  wait(20)
  for y = 0, 320, 6 do scroll(y) end
  -- Skeletons shimmering past the edge.
  page("loading")
  for y = 0, 300, 12 do scroll(y) end
end
-- A slider springing to its ends, and a checkbox's ring swept past the
-- edge: pressed (the focus, no ring), then Tab and Shift-Tab back (the
-- ring).
page("inputs")
send({ kind = "zoom", change = { value = 1 } })
wait(3)
send({ kind = "zoom", change = { value = 0 } })
wait(3)
add(function() reveal = true; kui.dispatch({ kind = "audit_scroll" }) end)
wait(3)
add(function() kui.input.click(box.x + 8, box.y + box.h / 2) end)
wait(3)
add(function() pressed = true; kui.dispatch({ kind = "audit_scroll" }) end)
wait(2)
add(function() kui.input.key("tab") end)
wait(1)
add(function() kui.input.key("tab", { shift = true }) end)
wait(2)
add(function() print("ringed", ringed, tostring(pressed), box and box.y) end)
for y = 0, 1400, 7 do scroll(y) end
-- Toasts arriving and leaving, a menu and a dialog open.
page("overlays")
send({ kind = "toast_all" })
wait(10)
add(function() kui.dispatch({ kind = "toast_off", id = model.toast_n }) end)
wait(3)
add(function() kui.dispatch({ kind = "toast_off", id = model.toast_n - 1 }) end)
wait(10)
for y = 0, 600, 20 do scroll(y) end
scroll(0)
send({ kind = "menu", open = true })
wait(3)
send({ kind = "menu", open = false })
send({ kind = "popover", open = true })
wait(3)
send({ kind = "popover", open = false })
send({ kind = "dialog", open = true })
wait(5)
local i = 0
kui.timer.every(30, function()
  i = i + 1
  if steps[i] then steps[i]() else kui.exit(0) end
end)
"##;

/// Runs the audit walk at `scale` and checks that nothing escaped: every
/// quad, glyph and mask a node drew inside a box that clips or scrolls
/// stayed inside it (ADR 0074), the audit having looked at many frames.
fn kit_paint_audit(name: &str, scale: &str) {
    let dir = kit_app(name, KIT_AUDIT_WALK);
    // Its output to a file, not a pipe: a walk that finds escapes writes a
    // line per node, past what a pipe holds while nothing reads it, and
    // would block there until killed.
    let log = dir.join("walk.log");
    let mut child = Command::new(KUI)
        .args(["run", "--headless", "--scale", scale, "walk.lua"])
        .current_dir(&dir)
        .env("KUI_CACHE_DIR", dir.join(".cache"))
        .env("KUI_PAINT_AUDIT", "1")
        .env_remove("KUI_FLAGS")
        .env_remove("KUI_LOG")
        .stdout(std::fs::File::create(&log).unwrap())
        .stderr(std::fs::File::options().append(true).open(&log).unwrap())
        .spawn()
        .expect("run kui");
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break Some(status);
        }
        if start.elapsed() > Duration::from_secs(240) {
            let _ = child.kill();
            break None;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let s = std::fs::read_to_string(&log).unwrap();
    assert!(status.is_some_and(|s| s.success()), "{s}");
    let escapes: Vec<&str> = s.lines().filter(|l| l.contains("[paint-escape]")).collect();
    assert!(
        escapes.is_empty(),
        "{} node(s) drew outside the box they are in:\n{}",
        escapes.len(),
        escapes.join("\n")
    );
    assert!(
        s.contains("ringed\ttrue"),
        "Tab put the ring on the checkbox:\n{s}"
    );
    let line = s
        .lines()
        .find(|l| l.starts_with("kui: paint audit: "))
        .unwrap_or_else(|| panic!("no audit line:\n{s}"));
    let n: Vec<u64> = line
        .trim_start_matches("kui: paint audit: ")
        .split(", ")
        .map(|f| f.split(' ').next().unwrap().parse().unwrap())
        .collect();
    assert!(
        n[0] > 400 && n[1] > 100_000,
        "the audit looked at enough: {line}"
    );
    assert_eq!(n[2], 0, "{line}");
    assert!(!s.contains("warning"), "{s}");
}

#[test]
fn nothing_the_kit_app_draws_leaves_the_box_it_is_in() {
    kit_paint_audit("kit-app-audit", "1");
}

#[test]
fn nothing_the_kit_app_draws_leaves_the_box_it_is_in_at_a_fractional_scale() {
    kit_paint_audit("kit-app-audit-1-5", "1.5");
}

/// A walk of the Kit app driven by real input (`kui.input`): a view that
/// prints where focus is when asked (`probe(name)`) — the focused node's
/// label, or its key, and the page — and the live line chart wrapped in a
/// box of the app's own (`probe.chart`) whose rect it prints when asked
/// (`want_chart()`), so the walk can put the pointer on it.
const KIT_WALK_HEAD: &str = r##"
local kit = require("kit.ui")
local line_chart = kit.line_chart
kit.line_chart = function(p)
  if not p.focusable then return line_chart(p) end
  return column { key = "probe.chart", width = "grow", on_layout = { kind = "noop" }, line_chart(p) }
end
require("main")
model = init()
local inner, asked, chart_asked = view, nil, false
chart = nil
function probe(name) asked = name end
function want_chart() chart_asked = true end
function view(env)
  local t = inner(env)
  if chart_asked then
    chart, chart_asked = env.layout_of("probe.chart"), false
  end
  if asked then
    local f = env.focus
    print("focus", asked, model.page, f and (env.label_of(f) or tostring(f)) or "none")
    asked = nil
  end
  return t
end
local function run(steps)
  local i = 0
  kui.timer.every(120, function()
    i = i + 1
    if steps[i] then steps[i]() else kui.exit(0) end
  end)
end
"##;

/// What the walk printed: each `focus` line as (probe, page, focused).
fn focus_lines(s: &str) -> Vec<(String, String, String)> {
    s.lines()
        .filter_map(|l| l.strip_prefix("focus\t"))
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            (f[0].to_string(), f[1].to_string(), f[2].to_string())
        })
        .collect()
}

/// Tab and Shift-Tab, pressed as keys, walk the Kit app's controls: the
/// gallery's key sink (its page shortcut) is around them all, and it is no
/// Tab stop and keeps no Tab (backlog: the regression where Tab moved
/// nothing anywhere in the gallery). From launch, a page reached with ⌥↓,
/// and a press on a control — which gives the sink the keyboard — Tab goes
/// on from where the press was.
#[test]
fn the_kit_app_tabs_through_the_inputs_page_with_real_keys() {
    let dir = kit_app(
        "kit-app-tab",
        &format!(
            "{KIT_WALK_HEAD}{}",
            r##"
        run({
          function() kui.input.key("down", { alt = true }) end,
          function() probe("paged") end,
          function() kui.input.key("tab") end, function() probe("t1") end,
          function() kui.input.key("tab") end, function() probe("t2") end,
          function() kui.input.key("tab") end, function() probe("t3") end,
          function() kui.input.key("tab") end, function() probe("t4") end,
          function() kui.input.key("tab") end, function() probe("t5") end,
          function() kui.input.key("tab", { shift = true }) end, function() probe("back") end,
          function() kui.input.key("tab", { shift = true }) end, function() probe("back2") end,
        })
        "##
        ),
    );
    let o = kui(&dir, &["run", "--headless", "walk.lua"], 60);
    let s = out(&o);
    assert!(o.status.success(), "{s}");
    assert!(!s.contains("warning"), "{s}");
    let lines = focus_lines(&s);
    let at = |name: &str| {
        lines
            .iter()
            .find(|l| l.0 == name)
            .unwrap_or_else(|| panic!("no {name} in:\n{s}"))
            .clone()
    };
    assert_eq!(
        at("paged").1,
        "inputs",
        "⌥↓ from launch goes to the next page:\n{s}"
    );
    let tabs: Vec<String> = ["t1", "t2", "t3", "t4", "t5"]
        .iter()
        .map(|n| at(n).2)
        .collect();
    assert!(
        tabs.iter().all(|f| f != "none"),
        "each Tab lands somewhere: {tabs:?}"
    );
    let mut distinct = tabs.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(distinct.len(), tabs.len(), "each Tab moves on: {tabs:?}");
    assert_eq!(at("back").2, tabs[3], "Shift-Tab goes back one:\n{s}");
    assert_eq!(at("back2").2, tabs[2], "and another:\n{s}");
    assert!(
        lines.iter().all(|l| l.1 == "inputs"),
        "Tab pages nothing:\n{s}"
    );
}

/// The Charts page: Tab moves focus with the pointer over the live line
/// chart (its readout up), and after a click on the chart; the chart, a
/// Tab stop, steps its readout with the arrows and leaves ⌥↓ to the page
/// shortcut and Tab to the ring.
#[test]
fn the_kit_app_tabs_after_hovering_and_pressing_the_live_chart() {
    let dir = kit_app(
        "kit-app-chart-tab",
        &format!(
            "{KIT_WALK_HEAD}{}",
            r##"
        local function middle() return chart.x + chart.w / 2, chart.y + chart.h / 2 end
        run({
          function() kui.dispatch({ kind = "page", change = { value = "charts" } }) end,
          function() want_chart() end, function() end,
          function() kui.input.move(middle()) end, function() end,
          function() kui.input.key("tab") end, function() probe("h1") end,
          function() kui.input.key("tab") end, function() probe("h2") end,
          function() kui.input.key("tab") end, function() probe("h3") end,
          function() kui.input.click(middle()) end, function() probe("pressed") end,
          function() kui.input.key("tab") end, function() probe("after") end,
          function() kui.input.key("tab", { shift = true }) end, function() probe("chart") end,
          function() kui.input.key("left") end, function() probe("stepped") end,
          function() kui.input.key("down", { alt = true }) end, function() probe("paged") end,
          function() kui.input.key("up", { alt = true }) end, function() probe("back") end,
        })
        "##
        ),
    );
    let o = kui(&dir, &["run", "--headless", "walk.lua"], 60);
    let s = out(&o);
    assert!(o.status.success(), "{s}");
    assert!(!s.contains("warning"), "{s}");
    let lines = focus_lines(&s);
    let at = |name: &str| {
        lines
            .iter()
            .find(|l| l.0 == name)
            .unwrap_or_else(|| panic!("no {name} in:\n{s}"))
            .clone()
    };
    let hovered: Vec<String> = ["h1", "h2", "h3"].iter().map(|n| at(n).2).collect();
    let mut distinct = hovered.clone();
    distinct.dedup();
    assert!(
        hovered.iter().all(|f| f != "none") && distinct.len() == 3,
        "Tab moves on under a pointer over the chart: {hovered:?}"
    );
    let after = at("after").2;
    assert!(
        after != "none" && after != at("pressed").2,
        "Tab moves on after a press on the chart:\n{s}"
    );
    assert_eq!(
        at("chart").2,
        "plot",
        "Shift-Tab back onto the chart, a Tab stop:\n{s}"
    );
    assert_eq!(at("stepped").2, "plot", "an arrow keeps it there");
    assert_eq!(
        at("paged").1,
        "trace",
        "⌥↓ from the focused chart goes to the next page:\n{s}"
    );
    assert_eq!(at("back").1, "charts", "⌥↑ back:\n{s}");
}

/// ⌥↓ and ⌥↑, pressed as keys, go to the next and the previous page,
/// round the ends, from the key sink around the app; a text editor with
/// the keyboard keeps them, and another chord is not the shortcut.
#[test]
fn the_kit_app_pages_with_alt_up_and_down_but_not_in_an_editor() {
    let dir = kit_app(
        "kit-app-keys",
        &format!(
            "{KIT_WALK_HEAD}{}",
            r##"
        local function alt(code, shift) kui.input.key(code, { alt = true, shift = shift }) end
        -- The view, asked to move the keyboard on until a text editor has it.
        local walk, hunting = view, false
        function view(env)
          local t = walk(env)
          if hunting and not (env.focus and env.edit_text(env.focus)) then env.focus_next() end
          return t
        end
        run({
          function() alt("down") end, function() probe("p1") end,
          function() alt("down") end, function() probe("p2") end,
          function() alt("up") alt("up") alt("up") end, function() probe("p3") end,
          function() alt("down", true) end, function() probe("p4") end,
          function() alt("down") end, function() probe("p5") end,
          function() kui.dispatch({ kind = "page", change = { value = "inputs" } }) hunting = true end,
          function() end, function() end, function() end, function() end, function() end, function() end,
          function() hunting = false alt("down") end,
          function() probe("editing") end,
        })
        "##
        ),
    );
    let o = kui(&dir, &["run", "--headless", "walk.lua"], 60);
    let s = out(&o);
    assert!(o.status.success(), "{s}");
    assert!(!s.contains("warning"), "{s}");
    let lines = focus_lines(&s);
    let pages: Vec<&str> = lines
        .iter()
        .filter(|l| l.0.starts_with('p'))
        .map(|l| l.1.as_str())
        .collect();
    assert_eq!(
        pages,
        ["inputs", "data", "icons", "icons", "overview"],
        "⌥↓ on, ⌥↑ back round the first, ⇧⌥↓ nothing:\n{s}"
    );
    let editing = lines
        .iter()
        .find(|l| l.0 == "editing")
        .expect("the editing probe");
    assert_eq!(editing.1, "inputs", "a text field keeps ⌥↓:\n{s}");
}

/// Tab, pressed as a key, walks the Charts page's whole ring twice: as it
/// is, and with every chart drawn as an empty box. The stops the charts
/// add are the live line chart, which says `focusable = true` (its card
/// shows the arrows stepping the readout), and the bar chart whose bars
/// the app picks from (`on_select`): the sparkline, the histogram, the
/// lying bars — every chart that did not ask — are passed over.
#[test]
fn the_kit_app_tabs_past_every_chart_that_did_not_ask_to_be_a_stop() {
    let dir = kit_app(
        "kit-app-charts-tab",
        &format!(
            "{KIT_WALK_HEAD}{}",
            r##"
        no_charts = false
        for _, name in ipairs({ "line_chart", "sparkline", "histogram", "bar_chart" }) do
          local chart = kit[name]
          kit[name] = function(p) if no_charts then return row { width = 10, height = 10 } end return chart(p) end
        end
        local steps = { function() kui.dispatch({ kind = "page", change = { value = "charts" } }) end, function() end }
        for pass = 1, 2 do
          for i = 1, 40 do
            steps[#steps + 1] = function() kui.input.key("tab") end
            steps[#steps + 1] = function() probe("p" .. pass) end
          end
          steps[#steps + 1] = function() no_charts = true; kui.dispatch({ kind = "audit_scroll" }) end
          steps[#steps + 1] = function() end
        end
        run(steps)
        "##
        ),
    );
    let o = kui(&dir, &["run", "--headless", "walk.lua"], 120);
    let s = out(&o);
    assert!(o.status.success(), "{s}");
    assert!(!s.contains("warning"), "{s}");
    let lines = focus_lines(&s);
    let stops = |pass: &str| {
        let mut v: Vec<String> = lines
            .iter()
            .filter(|l| l.0 == pass)
            .map(|l| l.2.clone())
            .collect();
        assert!(
            v.len() == 40
                && lines
                    .iter()
                    .filter(|l| l.0 == pass)
                    .all(|l| l.1 == "charts"),
            "{pass}:\n{s}"
        );
        v.sort();
        v.dedup();
        v
    };
    let (with, without) = (stops("p1"), stops("p2"));
    let charts: Vec<&String> = with.iter().filter(|k| !without.contains(k)).collect();
    assert_eq!(
        charts,
        ["b1", "plot"],
        "the charts' stops: the bars to pick and the live line chart that asked, no other:\n{s}"
    );
    assert!(
        without.iter().all(|k| with.contains(k)),
        "the rest of the ring is the same:\n{s}"
    );
}

/// The Kit app describes its own design system (addons/kit/app/design):
/// the Foundations page draws the brand book, the Colours page every colour
/// role Kit exports now, with its usage note and following the Theme
/// panel's skin, the Type and Icons pages draw, ⌥↓ from the last card page
/// reaches them, and a card's Guide shows its components' guidelines.
#[test]
fn the_kit_app_describes_its_design_system() {
    let dir = kit_app(
        "kit-app-design",
        &format!(
            "{KIT_WALK_HEAD}{}",
            r##"
        local design = require("design")
        local inner2, said = view, nil
        function view(env)
          local t = inner2(env)
          if said then
            local live = design.live_colors(env)
            local n = 0
            for _ in pairs(live) do n = n + 1 end
            print("design", said, model.page, n, string.format("%08x", live.panel or 0),
              tostring(design.tokens() ~= nil), #design.icon_names())
            said = nil
          end
          return t
        end
        run({
          function() kui.dispatch({ kind = "page", change = { value = "keyboard" } }) end,
          function() kui.input.key("down", { alt = true }) end, function() probe("paged") end,
          function() said = "foundations" end,
          function() kui.dispatch({ kind = "page", change = { value = "colours" } }) end,
          function() said = "ledger" end,
          function() kui.dispatch({ kind = "theme", field = "skin", change = { value = "phosphor" } }) end,
          function() end, function() said = "phosphor" end,
          function() kui.dispatch({ kind = "page", change = { value = "type" } }) end, function() said = "type" end,
          function() kui.dispatch({ kind = "page", change = { value = "icons" } }) end, function() said = "icons" end,
          function() kui.dispatch({ kind = "page", change = { value = "inputs" } }) end,
          function() kui.dispatch({ kind = "guide", name = "inputs/buttons" }) end,
          function() print("guides", table.concat(design.guides_of(require("pages").DEMOS["inputs/buttons"].call), " "),
            tostring(model.guide["inputs/buttons"])) end,
        })
        "##
        ),
    );
    let o = kui(&dir, &["run", "--headless", "walk.lua"], 60);
    let s = out(&o);
    assert!(o.status.success(), "{s}");
    assert!(!s.contains("warning"), "{s}");
    let lines = focus_lines(&s);
    let paged = lines
        .iter()
        .find(|l| l.0 == "paged")
        .expect("the paged probe");
    assert_eq!(
        paged.1, "foundations",
        "⌥↓ from the last card page reaches the design system:\n{s}"
    );
    let design: Vec<Vec<&str>> = s
        .lines()
        .filter_map(|l| l.strip_prefix("design\t"))
        .map(|l| l.split('\t').collect())
        .collect();
    let at = |name: &str| {
        design
            .iter()
            .find(|d| d[0] == name)
            .unwrap_or_else(|| panic!("no {name}:\n{s}"))
            .clone()
    };
    for name in ["foundations", "ledger", "phosphor", "type", "icons"] {
        let d = at(name);
        assert!(
            d[2].parse::<usize>().unwrap() >= 60,
            "{name}: Kit's colour roles read live: {d:?}"
        );
        assert_eq!(d[4], "true", "{name}: design/tokens.json read: {d:?}");
        assert_eq!(
            d[5], "40",
            "{name}: the icon set read from the brand book: {d:?}"
        );
    }
    assert_ne!(
        at("ledger")[3],
        at("phosphor")[3],
        "the Colours page follows the skin:\n{s}"
    );
    let guides = s
        .lines()
        .find_map(|l| l.strip_prefix("guides\t"))
        .expect("the guides line");
    assert!(
        guides.starts_with("Button IconButton") && guides.ends_with("true"),
        "the Buttons card's guides open: {guides}"
    );
}

/// The Kit app's live feeds tick only while the page in view reads them:
/// on the Inputs page, Live on, no timer wakes the app and the view stands
/// still; the Charts page's chart ticks every 100 ms, the Data page's log
/// every 400 ms, kept to its last 500 lines, and Overview's sample count
/// once a second, ten samples at a time.
#[test]
fn the_kit_apps_feeds_tick_only_for_a_page_that_reads_them() {
    let dir = kit_app(
        "kit-app-feeds",
        r##"
        require("main")
        model = init()
        local live = require("demos.feeds")
        local views, inner = 0, view
        function view(env) views = views + 1 return inner(env) end
        local function rev(ch) return ch and ch.rev or -1 end
        local at = {}
        local function mark(name) at[name] = { views = views, chart = rev(model.chart), log = rev(model.log), n = #model.series.t } end
        local function say(name)
          local a = at[name]
          print(name, views - a.views, rev(model.chart) ~= a.chart, rev(model.log) ~= a.log, #model.series.t - a.n)
        end
        local plan = {
          { 1500, function() kui.dispatch({ kind = "page", change = { value = "inputs" } }) end },
          { 2000, function() mark("inputs") end },
          { 3500, function() say("inputs") kui.dispatch({ kind = "page", change = { value = "charts" } }) end },
          { 3700, function() mark("charts") end },
          { 4700, function() say("charts") kui.dispatch({ kind = "page", change = { value = "data" } }) end },
          { 4900, function() mark("data") end },
          { 5900, function() say("data") kui.dispatch({ kind = "page", change = { value = "overview" } }) end },
          { 6100, function() mark("overview") end },
          { 8200, function() say("overview") kui.dispatch({ kind = "live", toggle = { on = false } }) end },
          { 8400, function() mark("paused") end },
          { 9500, function() say("paused")
            -- The log keeps its last 500 lines.
            model.live = true
            for _ = 1, 600 do live.log_line(model) end
            print("lines", #model.log, #model.log_lines.time)
            kui.exit(0)
          end },
        }
        for _, step in ipairs(plan) do kui.timer.after(step[1], step[2]) end
        "##,
    );
    let o = kui(&dir, &["run", "--headless", "walk.lua"], 60);
    let s = out(&o);
    assert!(o.status.success(), "{s}");
    assert!(!s.contains("warning"), "{s}");
    let line = |name: &str| -> Vec<String> {
        s.lines()
            .find_map(|l| l.strip_prefix(&format!("{name}\t")))
            .unwrap_or_else(|| panic!("no {name} line:\n{s}"))
            .split('\t')
            .map(str::to_string)
            .collect()
    };
    let views = |name: &str| line(name)[0].parse::<u32>().unwrap();
    let (inputs, charts, data, overview, paused) = (
        line("inputs"),
        line("charts"),
        line("data"),
        line("overview"),
        line("paused"),
    );
    // Over 1.5 s on Inputs the worker's rows may still land, and a cancelled
    // timer wakes once more to end: three views at most.
    assert!(
        views("inputs") <= 3 && inputs[1] == "false" && inputs[2] == "false",
        "nothing ticks on Inputs: {inputs:?}\n{s}"
    );
    assert!(
        views("charts") >= 3 && charts[1] == "true" && charts[2] == "false",
        "the chart ticks on Charts, the log not: {charts:?}"
    );
    assert!(
        data[1] == "false" && data[2] == "true",
        "the log ticks on Data, the chart not: {data:?}"
    );
    let n: i64 = overview[3].parse().unwrap();
    assert!(
        views("overview") <= 4 && overview[2] == "false" && n >= 10 && n % 10 == 0,
        "Overview counts ten samples a second: {overview:?}"
    );
    // The cancelled timer wakes once more to end, as on Inputs.
    assert!(
        views("paused") <= 2 && paused[1] == "false" && paused[3] == "0",
        "Live off on Overview, nothing ticks: {paused:?}"
    );
    assert_eq!(
        line("lines"),
        ["500", "500"],
        "the log keeps its last 500 lines"
    );
}

/// Kit's design system docs stay true to Kit: a guideline for every
/// component of the schema and none for a component Kit lacks; the
/// brand book's icon set Kit's own; and tokens.json's colour roles and
/// values what Kit computes in every theme (design/tokens.lua's check,
/// which `-- --write` turns into a rewrite of the file).
#[test]
fn kits_design_docs_match_kit() {
    let kit = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../addons/kit");
    let schema = std::fs::read_to_string(kit.join("schema.lua")).unwrap();
    let components: Vec<String> = schema
        .lines()
        .filter_map(|l| l.strip_prefix("    { \""))
        .filter_map(|l| l.split('"').next())
        .map(|n| {
            n.split('_')
                .map(|w| {
                    let mut c = w.chars();
                    c.next()
                        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                        .unwrap_or_default()
                })
                .collect::<String>()
        })
        .collect();
    let mut guides: Vec<String> = std::fs::read_dir(kit.join("app/design/components"))
        .unwrap()
        .filter_map(|e| {
            e.ok()?
                .file_name()
                .to_str()?
                .strip_suffix(".md")
                .map(str::to_string)
        })
        .collect();
    guides.sort();
    let mut want = components.clone();
    want.sort();
    let missing: Vec<&String> = want.iter().filter(|c| !guides.contains(c)).collect();
    let stray: Vec<&String> = guides.iter().filter(|g| !want.contains(g)).collect();
    assert!(
        missing.is_empty(),
        "components with no guideline in app/design/components: {missing:?}"
    );
    assert!(
        stray.is_empty(),
        "guidelines for no component of the schema: {stray:?}"
    );

    let icons_src = std::fs::read_to_string(kit.join("src/icons.tl")).unwrap();
    let body = icons_src
        .split("local BODY")
        .nth(1)
        .unwrap()
        .split("\n}")
        .next()
        .unwrap();
    let mut icons: Vec<&str> = body
        .lines()
        .filter_map(|l| l.trim_start().split_once(" = "))
        .map(|(n, _)| n.trim())
        .filter(|n| n.chars().all(|c| c.is_ascii_lowercase() || c == '_'))
        .collect();
    icons.sort();
    let book = std::fs::read_to_string(kit.join("app/design/README.md")).unwrap();
    let line = book
        .lines()
        .find(|l| l.starts_with("- The set: "))
        .expect("the brand book's icon set");
    let mut listed: Vec<&str> = line.split('`').skip(1).step_by(2).collect();
    listed.sort();
    assert_eq!(listed, icons, "the brand book lists Kit's icons");

    let o = kui(
        &kit.join("app/design"),
        &["run", "--headless", "tokens.lua", "--", "--check"],
        60,
    );
    let s = out(&o);
    assert!(
        o.status.success() && s.contains("tokens.json matches Kit"),
        "{s}"
    );
}

/// The Kit app is Kit's gallery: every component its schema describes is
/// placed on one of its pages (by its wrapper, `kit.<name>(`), so a new
/// component cannot ship without a live demo.
#[test]
fn the_kit_app_places_every_component() {
    let kit = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../addons/kit");
    let schema = std::fs::read_to_string(kit.join("schema.lua")).unwrap();
    let mut rels = Vec::new();
    lua_files(&kit.join("app"), Path::new(""), &mut rels);
    let app = rels
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
    let missing: Vec<&&str> = names
        .iter()
        .filter(|n| !app.contains(&format!("kit.{n}(")))
        .collect();
    assert!(
        missing.is_empty(),
        "components with no demo in the Kit app: {missing:?}"
    );
    // And it runs as an app of its own: a name, an id, a window.
    let config = std::fs::read_to_string(kit.join("app/kui.config.lua")).unwrap();
    assert!(
        config.contains("name = \"Kit\"") && config.contains("id = \"dev.kui.kit.gallery\""),
        "{config}"
    );
}

/// Runs a Kit sample's own headless test (addons/kit/samples/<name>/test),
/// in place: each writes only to the temp folder and serves on a free port.
fn kit_sample(name: &str) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../addons/kit/samples")
        .join(name);
    let o = kui(&dir, &["run", "test", "--headless"], 60);
    let s = out(&o);
    assert!(
        o.status.success() && s.contains("PASS: 0 failure(s)"),
        "{name}:\n{s}"
    );
    assert!(!s.contains("warning"), "{name}:\n{s}");
}

/// The book's runner chapter (docs/book/src/runner.md) ends with this app
/// and its test, so the chapter cannot drift from what runs.
#[test]
fn the_books_task_list_passes_its_own_test() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/lua/tutorial/tasks");
    let o = kui(&dir, &["run", "test", "--headless"], 60);
    let s = out(&o);
    assert!(
        o.status.success() && s.contains("PASS: 0 failure(s)"),
        "{s}"
    );
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
    for f in [
        ".kui/types/kui.d.tl",
        ".kui/types/kit/ui.d.tl",
        ".kui/types/kui.lua",
        ".kui/types/kui_runner.lua",
        ".kui/types/kit/ui.lua",
        ".luarc.json",
        "tlconfig.lua",
    ] {
        assert!(dir.join(f).exists(), "{f}");
    }
    let decls = std::fs::read_to_string(dir.join(".kui/types/kui.d.tl")).unwrap();
    assert!(
        decls.contains("serve: function(opts?: ServeOpts): Server"),
        "the runner's API is in the kui record"
    );
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
    let o = kui(
        &dir,
        &["run", "--headless", "--", "--port", "3001", "x y"],
        20,
    );
    let s = out(&o);
    assert!(s.contains("args\t--port,3001,x y"), "{s}");
    assert!(s.contains("cwd\ttrue"), "{s}");
    assert!(s.contains("platform\ttrue\ttrue\ttrue"), "{s}");
    assert!(
        s.contains("create\ttrue\tfalse\ta"),
        "a second create leaves the first's file:\n{s}"
    );
}

#[cfg(unix)]
#[test]
fn a_group_dies_whole_and_spawned_processes_end_with_the_app() {
    // The last spawn is `exec sleep`: dash (Ubuntu's sh) keeps `sh -c` as
    // the sleep's parent where macOS's sh execs, and a process outside a
    // group ends with the app by its own pid alone.
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
                  kui.process.spawn("sh", { "-c", "exec sleep 31338" })
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
    assert!(
        s.contains("orphans\t0"),
        "the group's child died with it:\n{s}"
    );
    assert!(
        s.contains("after\tfalse"),
        "an exited process is not signalled again:\n{s}"
    );
    std::thread::sleep(Duration::from_millis(200));
    let left = Command::new("pgrep")
        .args(["-f", "sleep 31338"])
        .output()
        .unwrap();
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
    let bad = app(
        "bad-manifest",
        &[
            ("kui.config.lua", r#"return { chrome = "glass" }"#),
            ("main.lua", ""),
        ],
    );
    let o = kui(&bad, &["run", "--headless"], 20);
    assert!(
        out(&o).contains("native, custom or borderless"),
        "{}",
        out(&o)
    );
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
    let o = kui(
        &dir,
        &[
            "run",
            "--",
            "-p",
            "8080",
            "--dry-run",
            "--tag",
            "a",
            "--tag=b",
            "--mode=slow",
            "x",
            "y",
        ],
        20,
    );
    let s = out(&o);
    assert!(
        s.contains(r#"got	8080	127.0.0.1	true	["a","b"]	slow	["x","y"]"#),
        "{s}"
    );
    let o = kui(&dir, &["run"], 20);
    assert!(
        out(&o).contains(r#"got	3000	127.0.0.1	false	[]	fast	[]"#),
        "{}",
        out(&o)
    );

    // Help and mistakes end the app before its window, with no error of the runner's.
    let o = kui(&dir, &["run", "--", "--help"], 20);
    let s = out(&o);
    assert!(o.status.success(), "{s}");
    assert!(
        s.contains("parse-args")
            && s.contains("a parser test")
            && s.contains("-p, --port <integer>"),
        "{s}"
    );
    assert!(!s.contains("kui:") && !s.contains("got"), "{s}");
    let o = kui(&dir, &["run", "--", "--prot", "1"], 20);
    let s = out(&o);
    assert_eq!(o.status.code(), Some(2), "{s}");
    assert!(
        s.contains("unknown option --prot (did you mean --port?)"),
        "{s}"
    );
    assert!(!s.contains("stack traceback"), "{s}");
    let o = kui(&dir, &["run", "--", "--mode", "medium"], 20);
    assert!(
        out(&o).contains("expected one of fast, slow"),
        "{}",
        out(&o)
    );
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
    assert!(
        s.contains("sha256\tba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"),
        "{s}"
    );
    assert!(
        s.contains("sha1\ta9993e364706816aba3e25717850c26c9cd0d89d"),
        "{s}"
    );
    assert!(s.contains("fnv\te71fa2190541574b"), "{s}");
    assert!(s.contains(r#"array	{"empty":[],"map":{}}"#), "{s}");
    assert!(
        s.contains(r#"round	{"list":[],"obj":{}}"#),
        "an empty list decoded stays a list:\n{s}"
    );
    assert!(s.contains("[warn] careful\t1"), "{s}");
    assert!(
        s.contains("walk\tsrc/a.txt,src/deep/b.txt"),
        "node_modules is skipped:\n{s}"
    );
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
    let o = kui(
        &dir,
        &["run", "--shot", shot.to_str().unwrap(), "--after", "300"],
        30,
    );
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
                ".kui/docs/props.md",
                "written for the editor, not packed (ADR 0071)",
            ),
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
                print("kui dir", kui.fs.exists(".kui/docs/props.md"))
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
    let o = kui_env(
        &dir,
        &["pack", "--format", "kui", "--out", "dist"],
        &[],
        &cache,
        60,
    );
    assert!(o.status.success(), "{}", out(&o));
    let archive = dir.join("dist/packer-2.0.0.kui");
    assert!(archive.is_file(), "{}", out(&o));
    let o = kui_env(
        &dir,
        &[
            "run",
            archive.to_str().unwrap(),
            "--headless",
            "--",
            "a",
            "b",
        ],
        &[],
        &cache,
        60,
    );
    let s = out(&o);
    assert!(o.status.success(), "{s}");
    assert!(s.contains("packed\ttrue\tfalse"), "{s}");
    assert!(s.contains("args\ta,b"), "{s}");
    assert!(s.contains("id\tdev.kui.test.packer\t2.0.0"), "{s}");
    assert!(s.contains("asset\tan asset"), "{s}");
    assert!(s.contains("module\ta module"), "{s}");
    assert!(s.contains("test dir\tfalse"), "{s}");
    assert!(
        s.contains("kui dir\tfalse"),
        ".kui/ stays out of the archive:\n{s}"
    );
    assert!(
        s.contains("write\tfalse\ttrue"),
        "a packed app's own files are read-only:\n{s}"
    );
}

#[test]
fn a_bytecode_archive_runs_and_bytecode_loads_nowhere_else() {
    let dir = packer("pack-bytecode");
    let cache = dir.join(".cache");
    let o = kui_env(
        &dir,
        &["pack", "--format", "kui", "--bytecode", "--out", "dist"],
        &[],
        &cache,
        60,
    );
    assert!(o.status.success(), "{}", out(&o));
    let archive = dir.join("dist/packer-2.0.0.kui");
    let o = kui_env(
        &dir,
        &["run", archive.to_str().unwrap(), "--headless"],
        &[],
        &cache,
        60,
    );
    let s = out(&o);
    assert!(
        s.contains("packed\ttrue\ttrue") && s.contains("module\ta module"),
        "{s}"
    );

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
    assert!(
        s.contains("load\ttrue") && s.contains("binary chunk"),
        "{s}"
    );
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
        &[
            "pack",
            "--target",
            "linux-x64",
            "--format",
            "bin",
            "--runner",
            KUI,
            "--out",
            "dist",
        ],
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
            (
                "kui.config.lua",
                r#"return { name = "NoId", kui = "0.0.1" }"#,
            ),
            ("main.lua", "print('ran') kui.exit(0)"),
        ],
    );
    let cache = dir.join(".cache");
    let o = kui_env(&dir, &["pack", "--format", "kui"], &[], &cache, 60);
    assert!(
        !o.status.success() && out(&o).contains("needs an id"),
        "{}",
        out(&o)
    );
    let o = kui_env(&dir, &["run", "--headless"], &[], &cache, 60);
    let s = out(&o);
    assert!(s.contains("ran") && s.contains("pins kui 0.0.1"), "{s}");
}

/// `kui docs` (ADR 0071): the carried docs searched in the terminal, an
/// ADR printed whole, and the book's address handed to the opener — here
/// `echo`, so no browser opens.
#[test]
fn docs_searches_what_the_runner_carries_and_prints_an_adr() {
    let dir = app("docs", &[]);
    std::fs::create_dir_all(&dir).unwrap();
    let o = kui(&dir, &["docs", "hover_bg"], 20);
    let s = out(&o);
    assert!(o.status.success(), "{s}");
    assert!(
        s.starts_with("hoverBg — props.md › Container props"),
        "the exact prop first:\n{s}"
    );
    let o = kui(
        &dir,
        &["docs", "how", "do", "I", "animate", "a", "removal"],
        20,
    );
    let s = out(&o);
    assert!(
        s.starts_with("How do I animate a removal? — howto.md"),
        "{s}"
    );
    let o = kui(&dir, &["docs", "kit.button"], 20);
    assert!(
        out(&o).starts_with("kit.button — Kit's component reference"),
        "{}",
        out(&o)
    );
    assert!(
        out(&o).contains("`on_press`"),
        "its props and events: {}",
        out(&o)
    );
    let o = kui(&dir, &["docs", "--adr", "71"], 20);
    let s = out(&o);
    assert!(
        o.status.success() && s.contains("# Docs that travel with kui"),
        "{s}"
    );
    let o = kui(&dir, &["docs", "--adr", "9999"], 20);
    assert!(
        !o.status.success() && out(&o).contains("no ADR 9999"),
        "{}",
        out(&o)
    );
    let o = kui(&dir, &["docs", "nothing-is-called-this"], 20);
    assert_eq!(o.status.code(), Some(1), "{}", out(&o));
    let o = Command::new(KUI)
        .arg("docs")
        .env("KUI_BROWSER", "echo")
        .current_dir(&dir)
        .output()
        .unwrap();
    let s = out(&o);
    assert!(
        o.status.success() && s.matches("https://kui-book.qxuken.dev").count() == 2,
        "said and opened:\n{s}"
    );
    let help = out(&kui(&dir, &["help"], 20));
    assert!(help.contains("kui docs [query]"), "{help}");
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Without `--docs` or `--agents`, `kui new` writes what it always did:
/// no docs, no agent file, no `tooling` (ADR 0071, decision 3).
#[test]
fn new_without_flags_writes_no_docs_or_agent_files() {
    let root = app("new-plain", &[]);
    std::fs::create_dir_all(&root).unwrap();
    let o = kui(&root, &["new", "plain"], 20);
    assert!(o.status.success(), "{}", out(&o));
    let dir = root.join("plain");
    assert!(dir.join(".kui/types/kui.lua").exists());
    for f in [".kui/docs", "AGENTS.md", "CLAUDE.md"] {
        assert!(!dir.join(f).exists(), "{f} was written unasked");
    }
    let manifest = std::fs::read_to_string(dir.join("kui.config.lua")).unwrap();
    assert!(!manifest.contains("tooling"), "{manifest}");
    assert_eq!(
        std::fs::read_to_string(dir.join(".kui/version"))
            .unwrap()
            .trim(),
        VERSION
    );
}

/// `--docs` writes `.kui/docs`, `--agents` the agent block too, and each
/// is remembered as `tooling` in the manifest — written into a new app's,
/// and placed in an existing one's with the rest of it untouched.
#[test]
fn docs_and_agents_write_them_and_the_manifests_tooling() {
    let root = app("new-tooling", &[]);
    std::fs::create_dir_all(&root).unwrap();
    let o = kui(&root, &["new", "docs", "--docs"], 20);
    assert!(o.status.success(), "{}", out(&o));
    let dir = root.join("docs");
    for f in [
        "props.md",
        "howto.md",
        "kit.md",
        "book/runner.md",
        "adr/0071-docs-that-travel-with-kui.md",
    ] {
        assert!(dir.join(".kui/docs").join(f).exists(), "{f}");
    }
    assert!(
        !dir.join("AGENTS.md").exists(),
        "--docs alone writes no agent block"
    );
    let manifest = std::fs::read_to_string(dir.join("kui.config.lua")).unwrap();
    assert!(
        manifest.contains("tooling = { docs = true },"),
        "{manifest}"
    );

    let o = kui(&root, &["new", "agents", "--agents"], 20);
    assert!(o.status.success(), "{}", out(&o));
    let dir = root.join("agents");
    assert!(
        dir.join(".kui/docs/props.md").exists(),
        "--agents implies --docs"
    );
    let block = std::fs::read_to_string(dir.join("AGENTS.md")).unwrap();
    assert!(
        block.starts_with("<!-- kui:begin") && block.trim_end().ends_with("<!-- kui:end -->"),
        "{block}"
    );
    for want in [
        VERSION,
        ".kui/types",
        ".kui/docs",
        "kui run --watch",
        "kui check",
        "Lua 5.5",
        "kit.ui",
        "Events are data",
    ] {
        assert!(block.contains(want), "{want} missing from:\n{block}");
    }
    let manifest = std::fs::read_to_string(dir.join("kui.config.lua")).unwrap();
    assert!(
        manifest.contains("tooling = { docs = true, agents = true },"),
        "{manifest}"
    );

    // An app of its own: `kui types --docs` adds one line to its manifest.
    let mine = "-- my app\nreturn {\n    name = \"mine\", -- the name\n    size = { w = 300, h = 200 },\n}\n";
    let dir = app(
        "types-tooling",
        &[("main.lua", "kui.exit(0)\n"), ("kui.config.lua", mine)],
    );
    let o = kui(&dir, &["types", "--docs"], 20);
    assert!(o.status.success(), "{}", out(&o));
    let manifest = std::fs::read_to_string(dir.join("kui.config.lua")).unwrap();
    assert_eq!(
        manifest,
        mine.replacen(
            "return {\n",
            "return {\n    tooling = { docs = true },\n",
            1
        )
    );
    let o = kui(&dir, &["types", "--agents"], 20);
    assert!(o.status.success(), "{}", out(&o));
    let manifest = std::fs::read_to_string(dir.join("kui.config.lua")).unwrap();
    assert_eq!(
        manifest,
        mine.replacen(
            "return {\n",
            "return {\n    tooling = { docs = true, agents = true },\n",
            1
        )
    );
    let o = kui(&dir, &["run", "--headless"], 20);
    assert!(o.status.success(), "the manifest still runs: {}", out(&o));
    let o = kui(&dir, &["run", "--docs"], 20);
    assert!(
        !o.status.success() && out(&o).contains("kui new's and kui types'"),
        "{}",
        out(&o)
    );
}

/// The agent block goes after what a `CLAUDE.md` the app already has
/// says, and is rewritten between its markers alone.
#[test]
fn the_agent_block_is_appended_to_claude_md_and_rewritten_in_place() {
    let mine = "# Mine\n\nKeep this.\n";
    let dir = app(
        "agents-claude",
        &[("main.lua", "kui.exit(0)\n"), ("CLAUDE.md", mine)],
    );
    let o = kui(&dir, &["types", "--agents"], 20);
    assert!(o.status.success(), "{}", out(&o));
    assert!(
        !dir.join("AGENTS.md").exists(),
        "the block went into CLAUDE.md"
    );
    let text = std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap();
    assert!(
        text.starts_with("# Mine\n\nKeep this.\n\n<!-- kui:begin"),
        "{text}"
    );
    // Written by an older runner, with more of the app's own after it.
    let old = text.replace(VERSION, "0.0.1") + "\nAfter the block.\n";
    std::fs::write(dir.join("CLAUDE.md"), &old).unwrap();
    std::fs::write(dir.join(".kui/version"), "0.0.1\n").unwrap();
    let o = kui(&dir, &["check"], 20);
    assert!(
        out(&o).contains("the agent block in CLAUDE.md"),
        "{}",
        out(&o)
    );
    let now = std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap();
    assert_eq!(now, text + "\nAfter the block.\n");
    assert_eq!(now.matches("<!-- kui:begin").count(), 1);
}

/// `.kui/` written by another runner is rewritten by `kui run` and `kui
/// check` — the types, and the docs where the app opted in — in one line,
/// once; an app that opted into nothing gets nothing (decision 6).
#[test]
fn a_version_change_refreshes_kui_and_creates_nothing_unasked() {
    let dir = app("refresh-types", &[("main.lua", "kui.exit(0)\n")]);
    assert!(kui(&dir, &["types"], 20).status.success());
    std::fs::write(dir.join(".kui/version"), "0.0.1\n").unwrap();
    std::fs::remove_file(dir.join(".kui/types/kui.lua")).unwrap();
    let o = kui(&dir, &["run", "--headless"], 20);
    let s = out(&o);
    assert!(o.status.success(), "{s}");
    assert_eq!(
        s.matches("kui: rewrote .kui/types for kui").count(),
        1,
        "{s}"
    );
    assert!(s.contains("(written by kui 0.0.1)"), "{s}");
    assert!(dir.join(".kui/types/kui.lua").exists());
    assert_eq!(
        std::fs::read_to_string(dir.join(".kui/version"))
            .unwrap()
            .trim(),
        VERSION
    );
    assert!(
        !dir.join(".kui/docs").exists() && !dir.join("AGENTS.md").exists(),
        "not opted into"
    );
    let o = kui(&dir, &["run", "--headless"], 20);
    assert!(!out(&o).contains("rewrote"), "once: {}", out(&o));

    // Opted into docs, as a fresh clone is: no .kui/ yet.
    let dir = app(
        "refresh-docs",
        &[
            ("main.lua", "kui.exit(0)\n"),
            ("kui.config.lua", "return { tooling = { docs = true } }\n"),
        ],
    );
    let o = kui(&dir, &["check"], 20);
    assert!(
        out(&o).contains("rewrote .kui/types and .kui/docs"),
        "{}",
        out(&o)
    );
    assert!(dir.join(".kui/docs/howto.md").exists() && dir.join(".kui/types/kui.d.tl").exists());
    assert!(!dir.join("AGENTS.md").exists());
    std::fs::write(dir.join(".kui/version"), "0.0.1\n").unwrap();
    std::fs::remove_file(dir.join(".kui/docs/howto.md")).unwrap();
    let o = kui(&dir, &["run", "--headless"], 20);
    assert!(
        out(&o).contains("rewrote .kui/types and .kui/docs"),
        "{}",
        out(&o)
    );
    assert!(dir.join(".kui/docs/howto.md").exists());

    // Nothing opted into and no .kui/: nothing written.
    let dir = app("refresh-none", &[("main.lua", "kui.exit(0)\n")]);
    let o = kui(&dir, &["run", "--headless"], 20);
    assert!(
        o.status.success() && !out(&o).contains("rewrote"),
        "{}",
        out(&o)
    );
    assert!(!dir.join(".kui").exists());
    let o = kui(&dir, &["check"], 20);
    assert!(
        o.status.success() && !dir.join(".kui").exists(),
        "{}",
        out(&o)
    );
}

/// A timer whose callback dispatches rings the loop once (backlog C141):
/// the dispatch is delivered by the drain the timer's own wake runs, and
/// asks for no second frame with nothing new in it.
#[test]
fn a_timer_that_dispatches_draws_one_frame_a_tick() {
    let dir = app(
        "timer-frames",
        &[(
            "main.lua",
            r#"
            function init() return { ticks = 0, views = 0 } end
            function update(msg)
              if msg.kind == "tick" then model.ticks = model.ticks + 1 end
              if model.ticks == 20 then
                print("frames", model.views)
                kui.exit(0)
              end
            end
            function view(env)
              model.views = model.views + 1
              return column { text("tick " .. model.ticks) }
            end
            ticker = kui.timer.every(25, function() kui.dispatch({ kind = "tick" }) end)
            "#,
        )],
    );
    let o = kui(&dir, &["run", "--headless", "."], 20);
    let s = out(&o);
    let frames: u32 = s
        .lines()
        .find_map(|l| l.strip_prefix("frames\t"))
        .and_then(|n| n.trim().parse().ok())
        .unwrap_or_else(|| panic!("no frame count in:\n{s}"));
    // Twenty ticks, the first frame, and slack for a tick landing while
    // a frame is owed; two a tick was forty.
    assert!(frames <= 26, "{frames} frames for 20 ticks:\n{s}");
}
