//! Scripted subprocesses retain the native process protocol and waits.
use super::super::{shared, tried};
use crate::{context::RunContext, ops::Ops};
use kui_core::Value;
use mlua::{Lua, UserData, UserDataFields, UserDataMethods};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
struct Line {
    at: f64,
    text: String,
}
pub struct MockProc {
    run: Arc<RunContext>,
    ops: Ops,
    out: Mutex<VecDeque<Line>>,
    err: Mutex<VecDeque<Line>>,
    end: f64,
    code: i64,
    killed: Arc<AtomicBool>,
    closed: AtomicBool,
}
fn lines(value: Option<&serde_json::Value>, start: f64) -> VecDeque<Line> {
    match value {
        Some(serde_json::Value::String(s)) => s
            .lines()
            .map(|text| Line {
                at: start,
                text: text.into(),
            })
            .collect(),
        Some(serde_json::Value::Array(rows)) => rows
            .iter()
            .map(|row| Line {
                at: start
                    + row
                        .get("at")
                        .and_then(serde_json::Value::as_f64)
                        .unwrap_or(0.0),
                text: row
                    .as_str()
                    .or_else(|| row.get("text").and_then(serde_json::Value::as_str))
                    .unwrap_or("")
                    .into(),
            })
            .collect(),
        _ => VecDeque::new(),
    }
}
impl MockProc {
    pub fn make(lua: &Lua, rule: serde_json::Value) -> mlua::Result<mlua::AnyUserData> {
        if let Some(error) = crate::effects::failure(&rule) {
            return Err(mlua::Error::runtime(error));
        }
        let s = shared(lua);
        let now = s.run.clock.virtual_time().unwrap_or(0.0);
        let response = rule.get("response").unwrap_or(&rule);
        let delay = rule
            .get("delay")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0);
        if !delay.is_finite() || delay < 0.0 {
            return Err(mlua::Error::runtime("invalid mock delay"));
        }
        lua.create_userdata(Self {
            run: s.run.clone(),
            ops: s.ops.clone(),
            out: Mutex::new(lines(response.get("stdout"), now)),
            err: Mutex::new(lines(response.get("stderr"), now)),
            end: now + delay,
            code: response
                .get("code")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0),
            killed: Arc::new(AtomicBool::new(false)),
            closed: AtomicBool::new(false),
        })
    }
    fn answer(&self, id: u64, at: f64, value: Value) {
        let killed = self.killed.clone();
        if let Some(now) = self.run.clock.virtual_time() {
            self.run
                .clock
                .schedule_with(&self.ops, id, (at - now).max(0.0), move || {
                    Ok(if killed.load(Ordering::Relaxed) {
                        Value::Null
                    } else {
                        value
                    })
                });
        } else {
            self.ops.answer(id, Ok(value));
        }
    }
}
impl UserData for MockProc {
    fn add_fields<F: UserDataFields<Self>>(f: &mut F) {
        f.add_field_method_get("pid", |_, _| Ok(0));
    }
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_method("__read", |_, this, stream: Option<String>| {
            let id = this.ops.id();
            let row = if stream.as_deref() == Some("stderr") {
                this.err.lock().unwrap().pop_front()
            } else {
                this.out.lock().unwrap().pop_front()
            };
            let (at, value) = row
                .map(|l| (l.at, Value::Str(l.text)))
                .unwrap_or((this.end, Value::Null));
            this.answer(id, at, value);
            Ok(id)
        });
        m.add_method("__wait", |_, this, ()| {
            let id = this.ops.id();
            let killed = this.killed.clone();
            let code = this.code;
            if let Some(now) = this.run.clock.virtual_time() {
                let delay = if killed.load(Ordering::Relaxed) {
                    0.0
                } else {
                    (this.end - now).max(0.0)
                };
                this.run.clock.schedule_with(&this.ops, id, delay, move || {
                    Ok(Value::Int(if killed.load(Ordering::Relaxed) {
                        -9
                    } else {
                        code
                    }))
                });
            } else {
                this.ops.answer(id, Ok(Value::Int(code)));
            }
            Ok(id)
        });
        m.add_method("try_write",|lua,this,data:mlua::LuaString| {
            use base64::Engine;
            this.run.record("process.stdin",None,serde_json::json!({"base64":base64::engine::general_purpose::STANDARD.encode(data.as_bytes())}));
            tried(lua,if this.closed.load(Ordering::Relaxed)||this.killed.load(Ordering::Relaxed){Err(super::super::fail_as("closed","the process's stdin is closed"))}else{Ok(())})
        });
        m.add_method("close", |_, this, ()| {
            this.closed.store(true, Ordering::Relaxed);
            Ok(())
        });
        m.add_method("kill", |_, this, sig: Option<mlua::Value>| {
            let sig = super::sig_name(sig)?;
            let alive = !this.killed.load(Ordering::Relaxed)
                && this.run.clock.virtual_time().unwrap_or(this.end) < this.end;
            if !alive || sig == "0" {
                return Ok(alive);
            }
            Ok(!this.killed.swap(true, Ordering::Relaxed))
        });
    }
}
