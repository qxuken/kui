//! Case-owned native effect rules. Workers share data, never Lua closures.
use crate::modules;
use crate::ops::Answer;
use serde_json::{Value as J, json};
use std::sync::Mutex;

#[derive(Default)]
pub struct Effects {
    recording: bool,
    rules: Mutex<Vec<(String, J)>>,
    policies: Mutex<std::collections::HashMap<String, bool>>,
    calls: Mutex<Vec<J>>,
}
impl Effects {
    pub fn new(recording: bool) -> Self {
        Self {
            recording,
            ..Default::default()
        }
    }
    pub fn add(&self, service: &str, rule: J) -> Result<(), String> {
        if !matches!(service, "http" | "process" | "fs" | "sqlite" | "secrets") {
            return Err(format!("no mock service {service:?}"));
        }
        if !rule.is_object() {
            return Err("mock rule must be a table".into());
        }
        fn delay(value: Option<&J>) -> Result<(), String> {
            if let Some(value) = value
                && !value.as_f64().is_some_and(|v| v.is_finite() && v >= 0.0)
            {
                return Err("mock delay/at must be finite and nonnegative".into());
            }
            Ok(())
        }
        delay(rule.get("delay"))?;
        for key in ["progress", "stdout", "stderr"] {
            let rows = if key == "progress" {
                rule.get(key)
            } else {
                rule.get("response").unwrap_or(&rule).get(key)
            };
            if let Some(J::Array(rows)) = rows {
                for row in rows {
                    delay(row.get("at"))?;
                }
            }
        }
        if service == "http"
            && let Some(response) = rule.get("response")
        {
            if !response.is_object() {
                return Err("HTTP mock response must be a table".into());
            }
            if response
                .get("status")
                .is_some_and(|s| !s.as_u64().is_some_and(|s| (100..=599).contains(&s)))
            {
                return Err("HTTP mock status must be an integer from 100 to 599".into());
            }
            if response.get("body").is_some_and(|v| !v.is_string()) {
                return Err("HTTP mock body must be a string".into());
            }
            if response.get("headers").is_some_and(|v| !v.is_object()) {
                return Err("HTTP mock headers must be a table".into());
            }
        }
        let passthrough = rule
            .get("passthrough")
            .and_then(J::as_bool)
            .unwrap_or(false);
        self.policies
            .lock()
            .unwrap()
            .insert(service.into(), passthrough);
        self.rules.lock().unwrap().push((service.into(), rule));
        Ok(())
    }
    pub fn intercept(&self, service: &str, op: &str, args: &J) -> Result<Option<J>, String> {
        if !self.recording {
            return Ok(None);
        }
        let mut rules = self.rules.lock().unwrap();
        let index = rules.iter().position(|(s, r)| {
            s == service
                && r.get("op").and_then(J::as_str).is_none_or(|v| v == op)
                && r.get("match").is_none_or(|m| subset(m, args))
                && ["url", "method", "command", "path", "sql"]
                    .iter()
                    .all(|key| r.get(*key).is_none_or(|v| args.get(*key) == Some(v)))
        });
        let matched = index.is_some();
        self.calls
            .lock()
            .unwrap()
            .push(json!({"service":service,"op":op,"args":args,"matched":matched}));
        if let Some(i) = index {
            let rule = rules[i].1.clone();
            if rule.get("once").and_then(J::as_bool) == Some(true) {
                rules.remove(i);
            }
            Ok(Some(rule))
        } else if self.policies.lock().unwrap().get(service) == Some(&false) {
            Err(format!(
                "[unexpected_call] unexpected {service}.{op}: {args}"
            ))
        } else {
            Ok(None)
        }
    }
    pub fn calls(&self, service: Option<&str>) -> J {
        J::Array(
            self.calls
                .lock()
                .unwrap()
                .iter()
                .filter(|v| service.is_none_or(|s| v["service"] == s))
                .cloned()
                .collect(),
        )
    }
}
fn subset(want: &J, actual: &J) -> bool {
    match (want, actual) {
        (J::Object(a), J::Object(b)) => a
            .iter()
            .all(|(k, v)| b.get(k).is_some_and(|x| subset(v, x))),
        _ => want == actual,
    }
}
pub fn failure(rule: &J) -> Option<String> {
    rule.get("error").and_then(J::as_str).map(|m| {
        format!(
            "[{}] {m}",
            rule.get("code").and_then(J::as_str).unwrap_or("failed")
        )
    })
}
pub fn check(lua: &mlua::Lua, service: &str, op: &str, args: J) -> mlua::Result<()> {
    let s = modules::shared(lua);
    s.run.record(
        "effect",
        s.worker.as_ref().map(|w| w.0),
        json!({"service":service,"op":op,"args":args}),
    );
    if let Some(rule) = s
        .run
        .effects
        .intercept(service, op, &args)
        .map_err(mlua::Error::runtime)?
        && let Some(error) = failure(&rule)
    {
        return Err(mlua::Error::runtime(error));
    }
    Ok(())
}
pub fn process_result(rule: &J) -> Answer {
    if let Some(e) = failure(rule) {
        return Err(e);
    }
    let mut result = json!({"code":0,"ok":true,"stdout":"","stderr":""});
    if let Some(value) = rule.get("response").or_else(|| rule.get("result")) {
        let Some(fields) = value.as_object() else {
            return Err("[invalid] process mock response must be a table".into());
        };
        result.as_object_mut().unwrap().extend(fields.clone());
    }
    result["ok"] = json!(result["code"].as_i64() == Some(0));
    kui_core::json::parse(&result.to_string()).map_err(|e| e.to_string())
}

pub fn complete(s: &modules::Shared, id: u64, ms: f64, answer: Answer) {
    if s.run.clock.virtual_time().is_some() {
        s.run.clock.schedule(&s.ops, id, ms, answer);
    } else {
        let ops = s.ops.clone();
        std::thread::spawn(move || {
            if ms > 0.0 {
                std::thread::sleep(std::time::Duration::from_secs_f64(ms / 1000.0));
            }
            ops.answer(id, answer);
        });
    }
}
