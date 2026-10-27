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

static STORE: Mutex<Store> = Mutex::new(Store {
    lines: std::collections::VecDeque::new(),
    rev: 0,
});

fn clock() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // UTC: the log tab is for ordering and gaps, not for a wall clock.
    let s = secs % 86_400;
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

/// A file the log also goes to (ADR 0068, decision 20): a packed GUI app's
/// `kui.data_dir/logs/<name>.log`, or `KUI_LOG=<path>`. Each run starts a
/// fresh file and keeps the last few; a run's file stops growing at a cap.
struct Sink {
    file: std::fs::File,
    written: u64,
    capped: bool,
}

static SINK: Mutex<Option<Sink>> = Mutex::new(None);

/// A run's file stops here, with a line saying so.
const CAP: u64 = 8 * 1024 * 1024;
/// Earlier runs' files kept beside this one: `name.1.log` … `name.3.log`.
const RUNS: usize = 3;

/// Starts writing the log to `path` too, after moving the files earlier
/// runs left down one.
pub fn to_file(path: &std::path::Path) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let numbered = |n: usize| {
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        path.with_file_name(format!("{stem}.{n}.log"))
    };
    let _ = std::fs::remove_file(numbered(RUNS));
    for n in (1..RUNS).rev() {
        let _ = std::fs::rename(numbered(n), numbered(n + 1));
    }
    let _ = std::fs::rename(path, numbered(1));
    let file = std::fs::File::create(path)?;
    *SINK.lock().unwrap() = Some(Sink {
        file,
        written: 0,
        capped: false,
    });
    Ok(())
}

fn to_sink(time: &str, level: &str, text: &str) {
    use std::io::Write;
    let mut sink = SINK.lock().unwrap();
    let Some(s) = sink.as_mut() else { return };
    if s.capped {
        return;
    }
    let line = format!("{time} {level:<5} {text}\n");
    if s.written + line.len() as u64 > CAP {
        let _ = writeln!(
            s.file,
            "{time} warn  the log reached {} MB; the rest of this run is not written",
            CAP >> 20
        );
        s.capped = true;
        return;
    }
    s.written += line.len() as u64;
    let _ = s.file.write_all(line.as_bytes());
}

pub fn push(level: &'static str, text: String) {
    let time = clock();
    to_sink(&time, level, &text);
    let mut st = STORE.lock().unwrap();
    st.lines.push_back(Line { time, level, text });
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
