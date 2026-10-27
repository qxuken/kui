//! `kui.process`: `run` waits for a command and hands back its code and
//! output; `spawn` hands back a handle whose output arrives a line at a
//! time (`p:read_line()` waits for the next), with `write`, `wait` and
//! `kill`. Every wait is an operation, so inside a task it yields.
//!
//! A spawned process ends with the app unless it was spawned `detach`;
//! one spawned `group` leads a process group of its own, and its `kill`
//! reaches everything it started (`npx` and the tool under it).

use std::collections::HashMap;
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::process::{ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};

use kui_core::Value;
use mlua::{Lua, Table, UserData, UserDataMethods};

use crate::ops::{self, Ops};

use super::{fail, shared, tried, try_fn};

struct Opts {
    cwd: Option<std::path::PathBuf>,
    env: Vec<(String, String)>,
    stdin: Option<Vec<u8>>,
    group: bool,
    detach: bool,
}

fn command(
    lua: &Lua,
    cmd: &str,
    args: Option<Vec<String>>,
    opts: Option<Table>,
) -> mlua::Result<(Command, Opts)> {
    let s = shared(lua);
    let mut c = Command::new(cmd);
    c.args(args.unwrap_or_default());
    let mut o = Opts {
        cwd: Some(s.dir.clone()),
        env: Vec::new(),
        stdin: None,
        group: false,
        detach: false,
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
        o.group = t.get::<Option<bool>>("group")?.unwrap_or(false);
        o.detach = t.get::<Option<bool>>("detach")?.unwrap_or(false);
    }
    #[cfg(unix)]
    if o.group {
        use std::os::unix::process::CommandExt;
        c.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: a console tool started from a window app opens
        // no console of its own.
        c.creation_flags(0x0800_0000);
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
    group: bool,
    state: Arc<Mutex<State>>,
    stdin: Arc<Mutex<Option<ChildStdin>>>,
    ops: Ops,
}

/// The processes that end with the app, by pid: each with whether it
/// leads a group.
#[derive(Default)]
pub struct Children(Mutex<HashMap<u32, bool>>);

impl Children {
    fn add(&self, pid: u32, group: bool) {
        self.0.lock().unwrap().insert(pid, group);
    }

    fn remove(&self, pid: u32) {
        self.0.lock().unwrap().remove(&pid);
    }

    /// Ends every one still running, groups whole.
    pub fn kill_all(&self) {
        for (pid, group) in self.0.lock().unwrap().drain() {
            let _ = signal(pid, "kill", group);
        }
    }
}

/// A signal by the name `kill` takes: `"term"` (or `"SIGTERM"`),
/// `"kill"`, `"int"`, `"hup"`, `"quit"`, `"usr1"`, `"usr2"`, or `0` to ask
/// whether it is alive.
#[cfg(unix)]
fn signum(name: &str) -> Option<libc::c_int> {
    let n = name
        .trim_start_matches("SIG")
        .trim_start_matches("sig")
        .to_ascii_lowercase();
    Some(match n.as_str() {
        "0" => 0,
        "term" => libc::SIGTERM,
        "kill" => libc::SIGKILL,
        "int" => libc::SIGINT,
        "hup" => libc::SIGHUP,
        "quit" => libc::SIGQUIT,
        "usr1" => libc::SIGUSR1,
        "usr2" => libc::SIGUSR2,
        _ => return None,
    })
}

/// Sends `sig` to `pid`, or to the group it leads: whether it arrived. A
/// process that exists but is not ours to signal counts as arrived for
/// `0` (it is alive).
#[cfg(unix)]
fn signal(pid: u32, sig: &str, group: bool) -> Result<bool, String> {
    let n = signum(sig).ok_or_else(|| format!("unknown signal {sig:?}"))?;
    let target = if group {
        -(pid as libc::pid_t)
    } else {
        pid as libc::pid_t
    };
    // SAFETY: kill(2) with a pid and a signal number.
    let r = unsafe { libc::kill(target, n) };
    Ok(r == 0 || (n == 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)))
}

/// Windows has no signals: any but `0` ends the process (its tree, for a
/// group), and `0` asks whether it runs.
#[cfg(windows)]
fn signal(pid: u32, sig: &str, group: bool) -> Result<bool, String> {
    use std::os::windows::process::CommandExt;
    let pid_s = pid.to_string();
    if sig == "0" {
        let out = kui_native::spawn::output(
            Command::new("tasklist")
                .args(["/FI", &format!("PID eq {pid}"), "/NH"])
                .creation_flags(0x0800_0000),
        )
        .map_err(|e| e.to_string())?;
        return Ok(String::from_utf8_lossy(&out.stdout)
            .split_whitespace()
            .any(|w| w == pid_s));
    }
    let mut args = vec!["/pid", &pid_s, "/F"];
    if group {
        args.push("/T");
    }
    // Its "SUCCESS: The process with PID …" lines are taskkill's, not the
    // app's: nothing of them reaches the app's own output (backlog F243).
    let status = kui_native::spawn::status(
        Command::new("taskkill")
            .args(args)
            .creation_flags(0x0800_0000)
            .stdout(Stdio::null())
            .stderr(Stdio::null()),
    )
    .map_err(|e| e.to_string())?;
    Ok(status.success())
}

#[cfg(not(any(unix, windows)))]
fn signal(_pid: u32, _sig: &str, _group: bool) -> Result<bool, String> {
    Err("signals are not supported on this platform".into())
}

fn command_line(pid: u32) -> Option<String> {
    #[cfg(windows)]
    let out = {
        use std::os::windows::process::CommandExt;
        kui_native::spawn::output(
            Command::new("powershell")
                .args([
                    "-NoProfile",
                    "-Command",
                    &format!(
                        "(Get-CimInstance Win32_Process -Filter \"ProcessId={pid}\").CommandLine"
                    ),
                ])
                .creation_flags(0x0800_0000),
        )
        .ok()?
    };
    #[cfg(not(windows))]
    let out = kui_native::spawn::output(Command::new("ps").args([
        "-o",
        "command=",
        "-p",
        &pid.to_string(),
    ]))
    .ok()?;
    let line = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !line.is_empty()).then_some(line)
}

/// `kill`'s argument: a name, or a number (`0`).
fn sig_name(v: Option<mlua::Value>) -> mlua::Result<String> {
    Ok(match v {
        None | Some(mlua::Value::Nil) => "kill".into(),
        Some(mlua::Value::Integer(i)) => i.to_string(),
        Some(mlua::Value::Number(n)) => (n as i64).to_string(),
        Some(mlua::Value::String(s)) => s.to_str()?.to_string(),
        Some(other) => {
            return Err(fail(format!(
                "a signal is a name, not a {}",
                other.type_name()
            )));
        }
    })
}

fn answer_line(ops: &Ops, id: u64, line: Option<String>) {
    ops.answer(id, Ok(line.map_or(Value::Null, Value::Str)));
}

fn reader(ops: Ops, state: Arc<Mutex<State>>, src: impl std::io::Read + Send + 'static, err: bool) {
    std::thread::spawn(move || {
        let mut r = BufReader::new(src);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            // Bytes, not a String: a line that is not UTF-8 (a tool's
            // progress bar cut mid-character) is read lossily rather than
            // ending the stream.
            let n = r.read_until(b'\n', &mut buf).unwrap_or(0);
            let mut st = state.lock().unwrap();
            let stream = if err { &mut st.err } else { &mut st.out };
            if n == 0 {
                stream.eof = true;
                for id in stream.waiting.drain(..) {
                    answer_line(&ops, id, None);
                }
                return;
            }
            let line = String::from_utf8_lossy(&buf)
                .trim_end_matches(['\n', '\r'])
                .to_string();
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
            let s = if stream.as_deref() == Some("stderr") {
                &mut st.err
            } else {
                &mut st.out
            };
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
                Some(code) => this
                    .ops
                    .answer(id, Ok(code.map_or(Value::Null, |c| Value::Int(c.into())))),
                None => st.exit_waiting.push(id),
            }
            Ok(id)
        });
        m.add_method("try_write", |lua, this, data: mlua::LuaString| {
            let mut stdin = this.stdin.lock().unwrap();
            tried(
                lua,
                match stdin.as_mut() {
                    Some(w) => w
                        .write_all(&data.as_bytes())
                        .map_err(|e| fail(format!("write: {e}"))),
                    None => Err(fail("the process's stdin is closed".into())),
                },
            )
        });
        m.add_method("close", |_, this, ()| {
            this.stdin.lock().unwrap().take();
            Ok(())
        });
        // `p:kill()` ends it; `p:kill("term")` asks it to. A group's
        // signal reaches the whole group.
        m.add_method("kill", |_, this, sig: Option<mlua::Value>| {
            let sig = sig_name(sig)?;
            // Held across the signal: the waiter records the exit under the
            // same lock, which leaves only the instant between its reap and
            // its record for a reused pid to be signalled.
            let st = this.state.lock().unwrap();
            if st.code.is_some() {
                return Ok(false);
            }
            signal(this.pid, &sig, this.group).map_err(fail)
        });
    }
}

pub fn install(lua: &Lua, kui: &Table) -> mlua::Result<()> {
    let process = lua.create_table()?;
    process.set(
        "__try_run",
        try_fn(
            lua,
            |lua, (cmd, args, opts): (String, Option<Vec<String>>, Option<Table>)| {
                let (mut c, o) = command(lua, &cmd, args, opts)?;
                let s = shared(lua);
                let id = s.ops.id();
                ops::run(&s.ops, id, move || {
                    c.stdin(if o.stdin.is_some() {
                        Stdio::piped()
                    } else {
                        Stdio::null()
                    });
                    c.stdout(Stdio::piped()).stderr(Stdio::piped());
                    let mut child =
                        kui_native::spawn::spawn(&mut c).map_err(|e| format!("{cmd}: {e}"))?;
                    if let Some(input) = o.stdin
                        && let Some(mut w) = child.stdin.take()
                    {
                        let _ = w.write_all(&input);
                    }
                    let out = child
                        .wait_with_output()
                        .map_err(|e| format!("{cmd}: {e}"))?;
                    Ok(Value::map([
                        (
                            "code",
                            out.status
                                .code()
                                .map_or(Value::Null, |c| Value::Int(c.into())),
                        ),
                        ("ok", Value::Bool(out.status.success())),
                        (
                            "stdout",
                            Value::Str(String::from_utf8_lossy(&out.stdout).into_owned()),
                        ),
                        (
                            "stderr",
                            Value::Str(String::from_utf8_lossy(&out.stderr).into_owned()),
                        ),
                    ]))
                });
                Ok(id)
            },
        )?,
    )?;
    process.set(
        "__try_spawn",
        try_fn(
            lua,
            |lua, (cmd, args, opts): (String, Option<Vec<String>>, Option<Table>)| {
                let (mut c, o) = command(lua, &cmd, args, opts)?;
                c.stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());
                let mut child =
                    kui_native::spawn::spawn(&mut c).map_err(|e| fail(format!("{cmd}: {e}")))?;
                let s = shared(lua);
                let state = Arc::new(Mutex::new(State::default()));
                reader(
                    s.ops.clone(),
                    state.clone(),
                    child.stdout.take().unwrap(),
                    false,
                );
                reader(
                    s.ops.clone(),
                    state.clone(),
                    child.stderr.take().unwrap(),
                    true,
                );
                let stdin = Arc::new(Mutex::new(child.stdin.take()));
                let pid = child.id();
                if !o.detach {
                    s.children.add(pid, o.group);
                }
                {
                    // The exit: a thread that owns the child and waits on it,
                    // so nothing polls; `kill` goes by pid.
                    let (state, ops, shared) = (state.clone(), s.ops.clone(), s.clone());
                    std::thread::spawn(move || {
                        let code = child.wait().ok().and_then(|st| st.code());
                        shared.children.remove(pid);
                        let mut st = state.lock().unwrap();
                        st.code = Some(code);
                        for id in st.exit_waiting.drain(..) {
                            ops.answer(id, Ok(code.map_or(Value::Null, |c| Value::Int(c.into()))));
                        }
                    });
                }
                Ok(Proc {
                    pid,
                    group: o.group,
                    state,
                    stdin,
                    ops: s.ops.clone(),
                })
            },
        )?,
    )?;
    // `kui.process.kill(pid, sig, group)`: a signal to a process this app
    // did not spawn, or did in an earlier run — `0` asks whether it is
    // alive. Whether it arrived.
    process.set(
        "kill",
        lua.create_function(
            |_, (pid, sig, group): (u32, Option<mlua::Value>, Option<bool>)| {
                signal(pid, &sig_name(sig)?, group.unwrap_or(false)).map_err(fail)
            },
        )?,
    )?;
    process.set("pid", std::process::id())?;
    // `kui.process.command_line(pid)` (waits): the command a process was
    // started with, nil when there is none — what tells a stale pid that
    // is still ours from one the OS gave to another program.
    process.set(
        "__command_line",
        lua.create_function(|lua, pid: u32| {
            let s = shared(lua);
            let id = s.ops.id();
            ops::run(&s.ops, id, move || {
                Ok(command_line(pid).map_or(Value::Null, Value::Str))
            });
            Ok(id)
        })?,
    )?;
    kui.set("process", process)?;
    Ok(())
}
