//! `kui.log` and `print`: lines the app writes, kept for the devtools'
//! Log tab as well as printed — one store for the process, so a worker's
//! lines land beside the app's, and the runner's own errors with them.

use std::sync::Mutex;

use mlua::{Lua, MultiValue, Table};

/// Lines kept; the oldest go first.
const KEEP: usize = 2000;

pub struct Line {
    pub time: String,
    pub level: &'static str,
    pub text: String,
}

struct Store {
    lines: std::collections::VecDeque<Line>,
    rev: u64,
}

static STORE: Mutex<Store> = Mutex::new(Store { lines: std::collections::VecDeque::new(), rev: 0 });

fn clock() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // UTC: the log tab is for ordering and gaps, not for a wall clock.
    let s = secs % 86_400;
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

pub fn push(level: &'static str, text: String) {
    let mut st = STORE.lock().unwrap();
    st.lines.push_back(Line { time: clock(), level, text });
    if st.lines.len() > KEEP {
        st.lines.pop_front();
    }
    st.rev += 1;
}

/// How many lines were ever written: what changes when there is more to show.
pub fn revision() -> u64 {
    STORE.lock().unwrap().rev
}

/// The kept lines, as three columns.
pub fn columns() -> (Vec<String>, Vec<&'static str>, Vec<String>) {
    let st = STORE.lock().unwrap();
    let mut t = Vec::with_capacity(st.lines.len());
    let mut l = Vec::with_capacity(st.lines.len());
    let mut m = Vec::with_capacity(st.lines.len());
    for line in &st.lines {
        t.push(line.time.clone());
        l.push(line.level);
        m.push(line.text.clone());
    }
    (t, l, m)
}

fn level_of(name: &str) -> &'static str {
    match name {
        "debug" => "debug",
        "warn" => "warn",
        "error" => "error",
        _ => "info",
    }
}

/// `kui.__log(level, ...)`: the values as `print` writes them, tab
/// between; printed to stderr with the level for a warning or an error,
/// to stdout otherwise, and kept.
pub fn install(lua: &Lua, kui: &Table) -> mlua::Result<()> {
    kui.set(
        "__log",
        lua.create_function(|lua, (level, rest): (String, MultiValue)| {
            let tostring: mlua::Function = lua.globals().get("tostring")?;
            let mut parts = Vec::with_capacity(rest.len());
            for v in rest {
                parts.push(tostring.call::<String>(v)?);
            }
            let text = parts.join("\t");
            let level = level_of(&level);
            match level {
                "warn" | "error" => eprintln!("[{level}] {text}"),
                _ => println!("{text}"),
            }
            push(level, text);
            Ok(())
        })?,
    )?;
    Ok(())
}
