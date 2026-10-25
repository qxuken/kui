//! `kui.process`: `run` waits for a command and hands back its code and
//! output; `spawn` hands back a handle whose output arrives a line at a
//! time (`p:read_line()` waits for the next), with `write`, `wait` and
//! `kill`. Every wait is an operation, so inside a task it yields.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};

use kui_core::Value;
use mlua::{Lua, Table, UserData, UserDataMethods};

use crate::ops::{self, Ops};

use super::{fail, shared};

struct Opts {
    cwd: Option<std::path::PathBuf>,
    env: Vec<(String, String)>,
    stdin: Option<Vec<u8>>,
}

fn command(lua: &Lua, cmd: &str, args: Option<Vec<String>>, opts: Option<Table>) -> mlua::Result<(Command, Opts)> {
    let s = shared(lua);
    let mut c = Command::new(cmd);
    c.args(args.unwrap_or_default());
    let mut o = Opts {
        cwd: Some(s.dir.clone()),
        env: Vec::new(),
        stdin: None,
    };
    if let Some(t) = opts {
        if let Some(cwd) = t.get::<Option<String>>("cwd")? {
            o.cwd = Some(s.path(&cwd));
        }
        if let Some(env) = t.get::<Option<Table>>("env")? {
            for pair in env.pairs::<String, String>() {
                o.env.push(pair?);
            }
        }
        if let Some(input) = t.get::<Option<mlua::LuaString>>("stdin")? {
            o.stdin = Some(input.as_bytes().to_vec());
        }
    }
    if let Some(cwd) = &o.cwd {
        c.current_dir(cwd);
    }
    for (k, v) in &o.env {
        c.env(k, v);
    }
    Ok((c, o))
}

#[derive(Default)]
struct Stream {
    lines: VecDeque<String>,
    eof: bool,
    waiting: VecDeque<u64>,
}

#[derive(Default)]
struct State {
    out: Stream,
    err: Stream,
    code: Option<Option<i32>>,
    exit_waiting: Vec<u64>,
}

struct Proc {
    pid: u32,
    state: Arc<Mutex<State>>,
    child: Arc<Mutex<Option<Child>>>,
    stdin: Arc<Mutex<Option<ChildStdin>>>,
    ops: Ops,
}

fn answer_line(ops: &Ops, id: u64, line: Option<String>) {
    ops.answer(id, Ok(line.map_or(Value::Null, Value::Str)));
}

fn reader(ops: Ops, state: Arc<Mutex<State>>, src: impl std::io::Read + Send + 'static, err: bool) {
    std::thread::spawn(move || {
        let mut r = BufReader::new(src);
        let mut buf = String::new();
        loop {
            buf.clear();
            let n = r.read_line(&mut buf).unwrap_or(0);
            let mut st = state.lock().unwrap();
            let stream = if err { &mut st.err } else { &mut st.out };
            if n == 0 {
                stream.eof = true;
                for id in stream.waiting.drain(..) {
                    answer_line(&ops, id, None);
                }
                return;
            }
            let line = buf.trim_end_matches(['\n', '\r']).to_string();
            match stream.waiting.pop_front() {
                Some(id) => answer_line(&ops, id, Some(line)),
                None => stream.lines.push_back(line),
            }
        }
    });
}

impl UserData for Proc {
    fn add_fields<F: mlua::UserDataFields<Self>>(f: &mut F) {
        f.add_field_method_get("pid", |_, this| Ok(this.pid));
    }

    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        // The id of an operation answered with the next line of `stream`
        // ("stdout" by default), nil at its end.
        m.add_method("__read", |_, this, stream: Option<String>| {
            let id = this.ops.id();
            let mut st = this.state.lock().unwrap();
            let s = if stream.as_deref() == Some("stderr") { &mut st.err } else { &mut st.out };
            if let Some(line) = s.lines.pop_front() {
                answer_line(&this.ops, id, Some(line));
            } else if s.eof {
                answer_line(&this.ops, id, None);
            } else {
                s.waiting.push_back(id);
            }
            Ok(id)
        });
        m.add_method("__wait", |_, this, ()| {
            let id = this.ops.id();
            let mut st = this.state.lock().unwrap();
            match st.code {
                Some(code) => this.ops.answer(id, Ok(code.map_or(Value::Null, |c| Value::Int(c.into())))),
                None => st.exit_waiting.push(id),
            }
            Ok(id)
        });
        m.add_method("write", |_, this, data: mlua::LuaString| {
            let mut stdin = this.stdin.lock().unwrap();
            match stdin.as_mut() {
                Some(w) => w.write_all(&data.as_bytes()).map_err(|e| fail(format!("write: {e}"))),
                None => Err(fail("the process's stdin is closed".into())),
            }
        });
        m.add_method("close", |_, this, ()| {
            this.stdin.lock().unwrap().take();
            Ok(())
        });
        m.add_method("kill", |_, this, ()| {
            if let Some(c) = this.child.lock().unwrap().as_mut() {
                let _ = c.kill();
            }
            Ok(())
        });
    }
}

pub fn install(lua: &Lua, kui: &Table) -> mlua::Result<()> {
    let process = lua.create_table()?;
    process.set(
        "__run",
        lua.create_function(|lua, (cmd, args, opts): (String, Option<Vec<String>>, Option<Table>)| {
            let (mut c, o) = command(lua, &cmd, args, opts)?;
            let s = shared(lua);
            let id = s.ops.id();
            ops::run(&s.ops, id, move || {
                c.stdin(if o.stdin.is_some() { Stdio::piped() } else { Stdio::null() });
                c.stdout(Stdio::piped()).stderr(Stdio::piped());
                let mut child = c.spawn().map_err(|e| format!("{cmd}: {e}"))?;
                if let Some(input) = o.stdin
                    && let Some(mut w) = child.stdin.take()
                {
                    let _ = w.write_all(&input);
                }
                let out = child.wait_with_output().map_err(|e| format!("{cmd}: {e}"))?;
                Ok(Value::map([
                    ("code", out.status.code().map_or(Value::Null, |c| Value::Int(c.into()))),
                    ("ok", Value::Bool(out.status.success())),
                    ("stdout", Value::Str(String::from_utf8_lossy(&out.stdout).into_owned())),
                    ("stderr", Value::Str(String::from_utf8_lossy(&out.stderr).into_owned())),
                ]))
            });
            Ok(id)
        })?,
    )?;
    process.set(
        "__spawn",
        lua.create_function(|lua, (cmd, args, opts): (String, Option<Vec<String>>, Option<Table>)| {
            let (mut c, _o) = command(lua, &cmd, args, opts)?;
            c.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
            let mut child = c.spawn().map_err(|e| fail(format!("{cmd}: {e}")))?;
            let s = shared(lua);
            let state = Arc::new(Mutex::new(State::default()));
            reader(s.ops.clone(), state.clone(), child.stdout.take().unwrap(), false);
            reader(s.ops.clone(), state.clone(), child.stderr.take().unwrap(), true);
            let stdin = Arc::new(Mutex::new(child.stdin.take()));
            let pid = child.id();
            let child = Arc::new(Mutex::new(Some(child)));
            {
                // The exit: polled, so `kill` can still reach the child.
                let (child, state, ops) = (child.clone(), state.clone(), s.ops.clone());
                std::thread::spawn(move || loop {
                    let done = child.lock().unwrap().as_mut().and_then(|c| c.try_wait().ok().flatten());
                    if let Some(status) = done {
                        let mut st = state.lock().unwrap();
                        st.code = Some(status.code());
                        for id in st.exit_waiting.drain(..) {
                            ops.answer(id, Ok(status.code().map_or(Value::Null, |c| Value::Int(c.into()))));
                        }
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(15));
                });
            }
            Ok(Proc { pid, state, child, stdin, ops: s.ops.clone() })
        })?,
    )?;
    kui.set("process", process)?;
    Ok(())
}
