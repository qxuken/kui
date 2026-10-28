//! A recording for a Lua script, with the `lua` feature (backlog F248, ADR
//! 0075 decisions 2, 4 and 8): Rust reads, keeps and analyses it; the script
//! is handed channels to draw from and small tables when it asks about one
//! frame, one span, or a range.
//!
//! ```lua
//! local rec = trace.open("session.kuitrace")       -- read whole, then watched
//! waterfall { bars = rec.spans, marks = rec.instants }
//! table { rows = rec.frames }
//! local f = rec:frame(row)                          -- verdict, phases, path …
//! if rec:poll() ~= "idle" then … end                -- what came since
//! rec:filter { origins = { "host" }, name = "rows", min_us = 50 }
//! ```
//!
//! The channels (ADR 0065, rows 0-based, times in ms on the recording's
//! clock):
//!
//! - `spans` — Kit's waterfall's `name`, `start`, `len`, `depth` (nesting
//!   within its track), `group` (0 kui, 1 the app, 2 on an extension each)
//!   and `target` (the node's key, hex, or empty); and `track`, `origin`,
//!   `self` (ms) and `frame` (the timeline's frame index it ran in or
//!   before). In the order the recording closed them, a child before its
//!   parent; each placed once its parent has come, so its depth is known.
//! - `frames` — `frame`, `index`, `begin`, `end`, `work`, the phases
//!   (`handler`, `build`, `devtools`, `layout`, `emit`, `render`, `wait`),
//!   `verdict`, `unchanged`, `burn`, `causes` and `nodes`; a frame is a row
//!   once the next begins (its present comes after its end), the last once
//!   the recording ends.
//! - `instants` — `at`, `label`, `kind` (`input`, `event`, `wake`,
//!   `request`, `mark`, `note`, `fresh`), `node`: the waterfall's `marks`.
//!
//! [`register`] makes the module; the host names it (the runner's
//! `kui.trace`). A file opened is watched (decision 6): the watch's thread
//! calls the host's `wake` with the `message` given to `open` — the runner
//! hands it to `update` — and `rec:poll()` reads what came, appended to
//! the channels with a filter applied to it, or starts again from a
//! rewritten file.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use kui_channel::lua::LuaChannel;
use kui_channel::{Channel, Column, Strs};
use mlua::{Lua, Table, UserData, UserDataFields, UserDataMethods, Value};

use crate::analysis::{self, Verdict};
use crate::{FileTransport, NameId, Origin, Polled, Record, Span, Stream, Trace, Watch};

/// What a host hands [`register`] to be woken when a watched file moves:
/// called on the watch's thread with the message the script gave `open`
/// (`{ message = … }`), for the host to hand its `update`, or none.
pub type Wake = Arc<dyn Fn(Option<&kui_value::Value>) + Send + Sync>;

/// How often a watched file is looked at (ADR 0075, decision 6).
const WATCH_EVERY: Duration = Duration::from_millis(250);

/// A span waiting for its parent longer than this many frames is placed
/// at the top: its parent began before the recording did.
const ORPHAN_FRAMES: usize = 2;

fn ms(us: u64) -> f64 {
    us as f64 / 1000.0
}

fn bad(e: impl std::fmt::Display) -> mlua::Error {
    mlua::Error::runtime(e.to_string())
}

/// The module: `open(path, { watch = true })`, `read(bytes)`.
pub fn register(lua: &Lua, wake: Option<Wake>) -> mlua::Result<Table> {
    let m = lua.create_table()?;
    m.set(
        "open",
        lua.create_function(move |_, (path, opts): (String, Option<Table>)| {
            let (watch, message) = match &opts {
                Some(o) => (
                    o.get::<Option<bool>>("watch")?.unwrap_or(true),
                    match o.get::<Value>("message")? {
                        Value::Nil => None,
                        v => Some(kui_channel::lua::from_lua(&v)?),
                    },
                ),
                None => (true, None),
            };
            Recording::open(&path, watch, wake.clone(), message)
        })?,
    )?;
    m.set(
        "read",
        lua.create_function(|_, bytes: mlua::LuaString| Recording::of_bytes(&bytes.as_bytes()))?,
    )?;
    Ok(m)
}

/// A span placed: its depth within its track and its timeline frame.
#[derive(Clone)]
struct Row {
    span: Span,
    depth: u32,
    frame: i64,
    self_us: u64,
}

/// What the views keep (decision 8): nothing filtered is everything.
#[derive(Clone, Debug, Default)]
struct Filter {
    origins: Option<Vec<String>>,
    name: Option<String>,
    min_us: u64,
    verdicts: Option<Vec<String>>,
    hide_unchanged: bool,
    /// `where`: a script's own test of a span, called from Rust with one
    /// table filled afresh for each (the query box's candidate, ADR 0075
    /// decision 8).
    predicate: Option<Predicate>,
}

/// A `where`: the script's function, the one table it is handed, and the
/// strings it reads made once a name (a span's name and origin).
#[derive(Clone, Debug)]
struct Predicate {
    test: mlua::Function,
    span: Table,
    lua: Lua,
    strings: std::cell::RefCell<Vec<Option<mlua::LuaString>>>,
    /// The table's keys, made once: a key spelled as a `&str` is made a
    /// Lua string again at every set.
    keys: [mlua::LuaString; 8],
}

impl Predicate {
    fn string(&self, id: usize, text: &str) -> mlua::Result<mlua::LuaString> {
        if let Some(Some(s)) = self.strings.borrow().get(id) {
            return Ok(s.clone());
        }
        let made = self.lua.create_string(text)?;
        let mut cache = self.strings.borrow_mut();
        if cache.len() <= id {
            cache.resize(id + 1, None);
        }
        cache[id] = Some(made.clone());
        Ok(made)
    }
}

impl Filter {
    fn is_empty(&self) -> bool {
        self.predicate.is_none()
            && self.origins.is_none()
            && self.name.is_none()
            && self.min_us == 0
            && self.verdicts.is_none()
            && !self.hide_unchanged
    }
}

/// A view of a channel: its identity, and while filtered the source rows
/// it holds.
struct View {
    shown: Channel,
    rows: Option<Vec<usize>>,
}

impl View {
    fn new(empty: &Channel) -> View {
        View {
            shown: Channel::table(empty_like(empty)).expect("an empty table"),
            rows: None,
        }
    }

    fn source_row(&self, row: usize) -> Option<usize> {
        match &self.rows {
            Some(rows) => rows.get(row).copied(),
            None => Some(row),
        }
    }
}

fn empty_like(ch: &Channel) -> Vec<(String, Column)> {
    ch.take_columns(&[]).expect("no rows taken")
}

/// A recording a script holds.
pub struct Recording {
    stream: Stream,
    transport: Option<FileTransport>,
    _watch: Option<Watch>,
    moved: Arc<AtomicBool>,
    ended: bool,
    budget: u64,
    seen: usize,
    rows: Vec<Row>,
    depth: HashMap<u32, u32>,
    pending: HashMap<u32, Vec<(Span, usize)>>,
    groups: Vec<NameId>,
    spans: Channel,
    frames: Channel,
    instants: Channel,
    frame_index: Vec<usize>,
    frame_unchanged: Vec<bool>,
    frame_verdict: Vec<&'static str>,
    spans_view: View,
    frames_view: View,
    filter: Filter,
    filter_error: Option<String>,
}

fn span_columns() -> Vec<(String, Column)> {
    vec![
        ("name".into(), Column::Str(Strs::default())),
        ("start".into(), Column::F64(Vec::new())),
        ("len".into(), Column::F64(Vec::new())),
        ("depth".into(), Column::I64(Vec::new())),
        ("group".into(), Column::I64(Vec::new())),
        ("target".into(), Column::Str(Strs::default())),
        ("track".into(), Column::I64(Vec::new())),
        ("origin".into(), Column::Str(Strs::default())),
        ("self".into(), Column::F64(Vec::new())),
        ("frame".into(), Column::I64(Vec::new())),
    ]
}

const PHASES: [&str; 7] = [
    "handler", "build", "devtools", "layout", "emit", "render", "wait",
];

fn frame_columns() -> Vec<(String, Column)> {
    let mut v: Vec<(String, Column)> = vec![
        ("frame".into(), Column::I64(Vec::new())),
        ("index".into(), Column::I64(Vec::new())),
        ("begin".into(), Column::F64(Vec::new())),
        ("end".into(), Column::F64(Vec::new())),
        ("work".into(), Column::F64(Vec::new())),
    ];
    for p in PHASES {
        v.push((p.into(), Column::F64(Vec::new())));
    }
    v.extend([
        ("verdict".into(), Column::Str(Strs::default())),
        ("unchanged".into(), Column::Bool(Vec::new())),
        ("burn".into(), Column::Bool(Vec::new())),
        ("causes".into(), Column::Str(Strs::default())),
        ("nodes".into(), Column::I64(Vec::new())),
    ]);
    v
}

fn instant_columns() -> Vec<(String, Column)> {
    vec![
        ("at".into(), Column::F64(Vec::new())),
        ("label".into(), Column::Str(Strs::default())),
        ("kind".into(), Column::Str(Strs::default())),
        ("node".into(), Column::Str(Strs::default())),
    ]
}

/// A span's colour: kui's, the app's, or one an extension each.
fn group(groups: &mut Vec<NameId>, o: Origin) -> i64 {
    match o {
        Origin::Kui => 0,
        Origin::Host => 1,
        Origin::Extension(ns) => {
            let k = match groups.iter().position(|g| *g == ns) {
                Some(k) => k,
                None => {
                    groups.push(ns);
                    groups.len() - 1
                }
            };
            2 + k as i64
        }
    }
}

fn origin_of(trace: &Trace, o: Origin) -> &str {
    match o {
        Origin::Kui => "kui",
        Origin::Host => "host",
        Origin::Extension(ns) => trace.name(ns),
    }
}

fn verdict_kind(v: &Verdict) -> &'static str {
    match v {
        Verdict::Late { .. } => "late",
        Verdict::Blocked { .. } => "blocked",
        Verdict::Slow { .. } => "slow",
        Verdict::Waited { .. } => "waited",
        Verdict::Wasted => "wasted",
        Verdict::Burn { .. } => "burn",
        Verdict::Fine { .. } => "fine",
    }
}

fn hex(node: u64) -> String {
    if node == 0 {
        String::new()
    } else {
        format!("{node:016x}")
    }
}

/// Columns being filled, by name.
struct Cols(Vec<(String, Column)>);

impl Cols {
    fn at(&mut self, i: usize) -> &mut Column {
        &mut self.0[i].1
    }
    fn f(&mut self, i: usize, x: f64) {
        if let Column::F64(v) = self.at(i) {
            v.push(x)
        }
    }
    fn i(&mut self, i: usize, x: i64) {
        if let Column::I64(v) = self.at(i) {
            v.push(x)
        }
    }
    fn b(&mut self, i: usize, x: bool) {
        if let Column::Bool(v) = self.at(i) {
            v.push(x)
        }
    }
    fn s(&mut self, i: usize, x: &str) {
        if let Column::Str(v) = self.at(i) {
            v.push(x)
        }
    }
}

impl Recording {
    fn new(transport: Option<FileTransport>, watch: Option<Watch>, moved: Arc<AtomicBool>) -> Self {
        let spans = Channel::table(span_columns()).expect("an empty table");
        let frames = Channel::table(frame_columns()).expect("an empty table");
        Recording {
            spans_view: View::new(&spans),
            frames_view: View::new(&frames),
            stream: Stream::new(),
            transport,
            _watch: watch,
            moved,
            ended: false,
            budget: 8_333,
            seen: 0,
            rows: Vec::new(),
            depth: HashMap::new(),
            pending: HashMap::new(),
            groups: Vec::new(),
            spans,
            frames,
            instants: Channel::table(instant_columns()).expect("an empty table"),
            frame_index: Vec::new(),
            frame_unchanged: Vec::new(),
            frame_verdict: Vec::new(),
            filter: Filter::default(),
            filter_error: None,
        }
    }

    /// The file at `path`, read whole, then watched when `watch`: a move
    /// calls `wake` with `message`.
    pub fn open(
        path: &str,
        watch: bool,
        wake: Option<Wake>,
        message: Option<kui_value::Value>,
    ) -> mlua::Result<Recording> {
        let transport =
            FileTransport::open(path, watch).map_err(|e| bad(format!("{path}: {e}")))?;
        let moved = Arc::new(AtomicBool::new(false));
        let w = if watch {
            let flag = moved.clone();
            Some(
                Watch::spawn(path, WATCH_EVERY, move || {
                    flag.store(true, Ordering::Relaxed);
                    if let Some(wake) = &wake {
                        wake(message.as_ref());
                    }
                })
                .map_err(|e| bad(format!("{path}: {e}")))?,
            )
        } else {
            None
        };
        let mut r = Recording::new(Some(transport), w, moved);
        r.poll_now().map_err(|e| bad(format!("{path}: {e}")))?;
        Ok(r)
    }

    /// A recording from its bytes, whole.
    pub fn of_bytes(bytes: &[u8]) -> mlua::Result<Recording> {
        let mut r = Recording::new(None, None, Arc::new(AtomicBool::new(false)));
        r.stream.push(bytes).map_err(bad)?;
        r.ended = true;
        r.absorb();
        Ok(r)
    }

    /// Reads what the file has: `"more"` and how many records, `"idle"`,
    /// `"ended"`, or `"reset"` for a recording begun again.
    fn poll_now(&mut self) -> Result<(&'static str, usize), String> {
        let Some(t) = self.transport.as_mut() else {
            return Ok(("ended", 0));
        };
        self.moved.store(false, Ordering::Relaxed);
        let mut came = 0;
        let mut reset = false;
        let state = loop {
            match self.stream.poll(t).map_err(|e| e.to_string())? {
                Polled::More(n) => came += n,
                Polled::Reset => reset = true,
                Polled::Idle => break "idle",
                Polled::Ended => break "ended",
            }
        };
        if reset {
            self.restart();
            came = self.stream.trace().records.len();
        }
        if state == "ended" {
            self.ended = true;
        }
        self.absorb();
        Ok(match (reset, came) {
            (true, _) => ("reset", came),
            (false, 0) => (state, 0),
            (false, n) => ("more", n),
        })
    }

    /// Everything placed is dropped: the file is another recording now.
    fn restart(&mut self) {
        let filter = std::mem::take(&mut self.filter);
        let (sv, fv) = (
            self.spans_view.shown.clone(),
            self.frames_view.shown.clone(),
        );
        let mut fresh = Recording::new(None, None, self.moved.clone());
        std::mem::swap(&mut fresh.stream, &mut self.stream);
        fresh.transport = self.transport.take();
        fresh._watch = self._watch.take();
        *self = fresh;
        // The views keep their identities, emptied: a script's handle stays.
        self.spans_view.shown = sv.publish_from(&self.spans);
        self.frames_view.shown = fv.publish_from(&self.frames);
        self.filter = filter;
    }

    fn origin_name(&self, o: Origin) -> &str {
        origin_of(self.stream.trace(), o)
    }

    /// Places `span` at `depth` and then the children that waited for it.
    fn place(&mut self, span: Span, depth: u32, out: &mut Vec<Row>) {
        let mut queue = vec![(span, depth)];
        // A child closes before its parent, so it waited for this one.
        while let Some((span, depth)) = queue.pop() {
            self.depth.insert(span.id, depth);
            if let Some(kids) = self.pending.remove(&span.id) {
                queue.extend(kids.into_iter().map(|(k, _)| (k, depth + 1)));
            }
            out.push(Row {
                span,
                depth,
                frame: -1,
                self_us: 0,
            });
        }
    }

    /// Takes in the records the stream read since the last time: spans
    /// whose parents have come, the instants, the frames finished; and the
    /// channels and views grown with them.
    fn absorb(&mut self) {
        if let Some(s) = &self.stream.trace().session {
            self.budget = ((s.budget_ms as f64) * 1000.0).max(1.0) as u64;
        }
        let frames_seen = self.stream.timeline().frames.len();
        let mut new_spans: Vec<Span> = Vec::new();
        let mut instants: Vec<(u64, String, &'static str, u64)> = Vec::new();
        {
            let trace = self.stream.trace();
            let names = |id| trace.name(id).to_string();
            for r in &trace.records[self.seen..] {
                match r {
                    Record::Span(s) => new_spans.push(s.clone()),
                    Record::Input { t, kind, count, .. } => {
                        let label = if *count > 1 {
                            format!("{} ×{count}", names(*kind))
                        } else {
                            names(*kind)
                        };
                        instants.push((*t, label, "input", 0));
                    }
                    Record::Event { t, kind, node } => {
                        instants.push((*t, format!("→ {}", names(*kind)), "event", *node))
                    }
                    Record::Wake { t, name } => {
                        instants.push((*t, format!("wake {}", names(*name)), "wake", 0))
                    }
                    Record::Request { t, why, .. } => {
                        instants.push((*t, format!("request {}", names(*why)), "request", 0))
                    }
                    Record::Mark { t, name, .. } => instants.push((*t, names(*name), "mark", 0)),
                    Record::Note { t, text } => instants.push((*t, text.clone(), "note", 0)),
                    Record::Placement {
                        t,
                        node,
                        component,
                        answer,
                        why,
                    } if names(*answer) != analysis::REPLAYED => {
                        let why = names(*why);
                        let label = if why.is_empty() {
                            format!("{} {}", names(*component), names(*answer))
                        } else {
                            format!("{} {} · {why}", names(*component), names(*answer))
                        };
                        instants.push((*t, label, "fresh", *node));
                    }
                    _ => {}
                }
            }
        }
        self.seen = self.stream.trace().records.len();
        let mut placed = Vec::new();
        let mut inst = Cols(instant_columns());
        for s in new_spans {
            if s.parent == 0 {
                self.place(s, 0, &mut placed);
            } else if let Some(&d) = self.depth.get(&s.parent) {
                self.place(s, d + 1, &mut placed);
            } else {
                self.pending
                    .entry(s.parent)
                    .or_default()
                    .push((s, frames_seen));
            }
        }
        // Spans whose parents never come: placed at the top.
        let orphans: Vec<u32> = self
            .pending
            .iter()
            .filter(|(_, kids)| {
                self.ended || kids.iter().any(|(_, at)| frames_seen > at + ORPHAN_FRAMES)
            })
            .map(|(p, _)| *p)
            .collect();
        for p in orphans {
            for (s, _) in self.pending.remove(&p).unwrap_or_default() {
                self.place(s, 0, &mut placed);
            }
        }
        // Spans, with their frame and self time.
        let tl = self.stream.timeline();
        let mut cols = Cols(span_columns());
        for row in &mut placed {
            let s = &row.span;
            row.frame = tl.frames.partition_point(|f| f.end_or_begin() < s.start) as i64;
        }
        for row in &mut placed {
            row.self_us = tl.self_us(&row.span);
        }
        let trace = self.stream.trace();
        for row in &placed {
            let self_us = row.self_us;
            let s = &row.span;
            let group = group(&mut self.groups, s.origin);
            let name = trace.name(s.name);
            let origin = origin_of(trace, s.origin);
            cols.s(0, name);
            cols.f(1, ms(s.start));
            cols.f(2, ms(s.dur));
            cols.i(3, row.depth as i64);
            cols.i(4, group);
            cols.s(5, &hex(s.node));
            cols.i(6, s.track.0 as i64);
            cols.s(7, origin);
            cols.f(8, ms(self_us));
            cols.i(9, row.frame);
        }
        let first_new_span = self.rows.len();
        self.rows.extend(placed);
        if self.rows.len() > first_new_span {
            self.spans = self
                .spans
                .append(cols.0, None)
                .expect("the spans' own columns");
        }
        // Instants, in time.
        instants.sort_by_key(|i| i.0);
        if !instants.is_empty() {
            for (t, label, kind, node) in &instants {
                inst.f(0, ms(*t));
                inst.s(1, label);
                inst.s(2, kind);
                inst.s(3, &hex(*node));
            }
            self.instants = self
                .instants
                .append(inst.0, None)
                .expect("the instants' columns");
        }
        // Frames finished: every one before the last, and the last once
        // the recording ended.
        let tl = self.stream.timeline();
        let trace = self.stream.trace();
        let done = if self.ended {
            tl.frames.len()
        } else {
            tl.frames.len().saturating_sub(1)
        };
        let first_new_frame = self.frame_index.len();
        if done > first_new_frame {
            let burns = analysis::burns(tl, trace);
            let in_burn = |i: usize| burns.iter().any(|b| (b.first..=b.last).contains(&i));
            let mut fc = Cols(frame_columns());
            for i in first_new_frame..done {
                let f = &tl.frames[i];
                let p = analysis::phases(tl, i, trace);
                let v = analysis::verdict(tl, i, self.budget, &burns, trace);
                let kind = verdict_kind(&v);
                fc.i(0, f.frame as i64);
                fc.i(1, i as i64);
                fc.f(2, ms(f.begin));
                fc.f(3, ms(f.end_or_begin()));
                fc.f(4, ms(f.work_us()));
                for (k, (_, us)) in p.named().iter().enumerate() {
                    fc.f(5 + k, ms(*us));
                }
                fc.s(12, kind);
                fc.b(13, f.unchanged == Some(true));
                fc.b(14, in_burn(i));
                let causes: Vec<&str> = f.causes.iter().map(|c| trace.name(*c)).collect();
                fc.s(15, &causes.join(", "));
                fc.i(16, f.nodes as i64);
                self.frame_index.push(i);
                self.frame_unchanged.push(f.unchanged == Some(true));
                self.frame_verdict.push(kind);
            }
            self.frames = self
                .frames
                .append(fc.0, None)
                .expect("the frames' own columns");
        }
        self.grow_views(first_new_span, first_new_frame);
    }

    fn keeps_span(&self, i: usize) -> mlua::Result<bool> {
        let f = &self.filter;
        let row = &self.rows[i];
        let s = &row.span;
        if s.dur < f.min_us {
            return Ok(false);
        }
        if let Some(origins) = &f.origins
            && !origins.iter().any(|o| o == self.origin_name(s.origin))
        {
            return Ok(false);
        }
        let trace = self.stream.trace();
        if let Some(name) = &f.name {
            let n = trace.name(s.name).to_lowercase();
            if !n.contains(&name.to_lowercase()) {
                return Ok(false);
            }
        }
        if let Some(p) = &f.predicate {
            let t = &p.span;
            let k = &p.keys;
            // Names by their ids from 2; kui's and the app's origins at 0
            // and 1.
            t.raw_set(&k[0], p.string(s.name as usize + 2, trace.name(s.name))?)?;
            let origin = match s.origin {
                Origin::Kui => p.string(0, "kui")?,
                Origin::Host => p.string(1, "host")?,
                Origin::Extension(ns) => p.string(ns as usize + 2, trace.name(ns))?,
            };
            t.raw_set(&k[1], origin)?;
            t.raw_set(&k[2], ms(s.start))?;
            t.raw_set(&k[3], ms(s.dur))?;
            t.raw_set(&k[4], ms(row.self_us))?;
            t.raw_set(&k[5], row.depth)?;
            t.raw_set(&k[6], s.track.0)?;
            t.raw_set(&k[7], row.frame)?;
            return p.test.call::<bool>(t);
        }
        Ok(true)
    }

    fn keeps_frame(&self, i: usize) -> bool {
        let f = &self.filter;
        if f.hide_unchanged && self.frame_unchanged[i] {
            return false;
        }
        match &f.verdicts {
            Some(v) => v.iter().any(|k| k == self.frame_verdict[i]),
            None => true,
        }
    }

    /// The views after new rows came: the whole shown again, or the new
    /// rows the filter keeps appended.
    fn grow_views(&mut self, from_span: usize, from_frame: usize) {
        if self.filter.is_empty() {
            self.spans_view.shown = self.spans_view.shown.publish_from(&self.spans);
            self.frames_view.shown = self.frames_view.shown.publish_from(&self.frames);
            self.spans_view.rows = None;
            self.frames_view.rows = None;
            return;
        }
        let mut spans = Vec::new();
        for i in from_span..self.rows.len() {
            match self.keeps_span(i) {
                Ok(true) => spans.push(i),
                Ok(false) => {}
                Err(e) => {
                    self.filter_error = Some(e.to_string());
                    break;
                }
            }
        }
        let frames: Vec<usize> = (from_frame..self.frame_index.len())
            .filter(|&i| self.keeps_frame(i))
            .collect();
        if !spans.is_empty() {
            let cols = self.spans.take_columns(&spans).expect("rows it holds");
            self.spans_view.shown = self
                .spans_view
                .shown
                .append(cols, None)
                .expect("its columns");
        }
        if !frames.is_empty() {
            let cols = self.frames.take_columns(&frames).expect("rows it holds");
            self.frames_view.shown = self
                .frames_view
                .shown
                .append(cols, None)
                .expect("its columns");
        }
        self.spans_view
            .rows
            .get_or_insert_with(Vec::new)
            .extend(spans);
        self.frames_view
            .rows
            .get_or_insert_with(Vec::new)
            .extend(frames);
    }

    /// Sets the filter and shows the rows it keeps; a `where` that throws
    /// leaves the views as they were.
    fn set_filter(&mut self, f: Filter) -> mlua::Result<()> {
        let was = std::mem::replace(&mut self.filter, f);
        self.filter_error = None;
        if self.filter.is_empty() {
            self.grow_views(self.rows.len(), self.frame_index.len());
            return Ok(());
        }
        let mut spans = Vec::new();
        for i in 0..self.rows.len() {
            match self.keeps_span(i) {
                Ok(true) => spans.push(i),
                Ok(false) => {}
                Err(e) => {
                    self.filter = was;
                    return Err(e);
                }
            }
        }
        let frames: Vec<usize> = (0..self.frame_index.len())
            .filter(|&i| self.keeps_frame(i))
            .collect();
        let sc = self.spans.take_columns(&spans).expect("rows it holds");
        let fcs = self.frames.take_columns(&frames).expect("rows it holds");
        self.spans_view.shown = self.spans_view.shown.publish(sc).expect("its columns");
        self.frames_view.shown = self.frames_view.shown.publish(fcs).expect("its columns");
        self.spans_view.rows = Some(spans);
        self.frames_view.rows = Some(frames);
        Ok(())
    }

    fn trace(&self) -> &Trace {
        self.stream.trace()
    }
}

fn filter_of(lua: &Lua, t: &Table) -> mlua::Result<Filter> {
    let list = |k: &str| -> mlua::Result<Option<Vec<String>>> {
        Ok(match t.get::<Option<Table>>(k)? {
            Some(l) => Some(l.sequence_values::<String>().collect::<mlua::Result<_>>()?),
            None => None,
        })
    };
    Ok(Filter {
        origins: list("origins")?,
        name: t.get::<Option<String>>("name")?.filter(|n| !n.is_empty()),
        min_us: t.get::<Option<f64>>("min_us")?.unwrap_or(0.0).max(0.0) as u64,
        verdicts: list("verdicts")?,
        hide_unchanged: t.get::<Option<bool>>("hide_unchanged")?.unwrap_or(false),
        predicate: match t.get::<Option<mlua::Function>>("where")? {
            Some(test) => Some(Predicate {
                test,
                span: lua.create_table_with_capacity(0, 8)?,
                lua: lua.clone(),
                strings: Default::default(),
                keys: [
                    lua.create_string("name")?,
                    lua.create_string("origin")?,
                    lua.create_string("start")?,
                    lua.create_string("len")?,
                    lua.create_string("self")?,
                    lua.create_string("depth")?,
                    lua.create_string("track")?,
                    lua.create_string("frame")?,
                ],
            }),
            None => None,
        },
    })
}

/// A frame's detail: what the Perf tab's opened frame shows.
fn frame_detail(lua: &Lua, r: &Recording, i: usize) -> mlua::Result<Table> {
    let tl = r.stream.timeline();
    let trace = r.trace();
    let f = &tl.frames[i];
    let name = |id| trace.name(id);
    let t = lua.create_table()?;
    t.set("frame", f.frame)?;
    t.set("index", i)?;
    t.set("begin", ms(f.begin))?;
    t.set("finish", f.end.map(ms))?;
    t.set("gap", ms(f.begin.saturating_sub(f.gap_from)))?;
    t.set("work", ms(f.work_us()))?;
    t.set("nodes", f.nodes)?;
    t.set("unchanged", f.unchanged)?;
    let phases = lua.create_table()?;
    for (k, us) in analysis::phases(tl, i, trace).named() {
        phases.set(k, ms(us))?;
    }
    t.set("phases", phases)?;
    let burns = analysis::burns(tl, trace);
    let v = analysis::verdict(tl, i, r.budget, &burns, trace);
    let vt = lua.create_table()?;
    vt.set("kind", verdict_kind(&v))?;
    match v {
        Verdict::Late { wait, slept } => {
            vt.set("wait", ms(wait))?;
            vt.set("slept", slept)?;
        }
        Verdict::Blocked { busy } => vt.set("busy", ms(busy))?,
        Verdict::Slow {
            work,
            phase,
            phase_us,
            span,
        } => {
            vt.set("work", ms(work))?;
            vt.set("phase", phase)?;
            vt.set("phase_ms", ms(phase_us))?;
            if let Some((n, us)) = span {
                vt.set("span", name(n))?;
                vt.set("span_ms", ms(us))?;
            }
        }
        Verdict::Waited { wall, cpu } => {
            vt.set("wall", ms(wall))?;
            vt.set("cpu", ms(cpu))?;
        }
        Verdict::Wasted => {}
        Verdict::Burn { k, of } => {
            vt.set("k", k)?;
            vt.set("of", of)?;
        }
        Verdict::Fine { share } => vt.set("share", share)?,
    }
    t.set("verdict", vt)?;
    t.set(
        "causes",
        lua.create_sequence_from(f.causes.iter().map(|c| name(*c)))?,
    )?;
    let holders = lua.create_table()?;
    for h in &f.holders {
        let x = lua.create_table()?;
        x.set("what", name(h.what))?;
        x.set("name", name(h.name))?;
        x.set("node", hex(h.node))?;
        holders.push(x)?;
    }
    t.set("holders", holders)?;
    if let Some(p) = analysis::input_path(tl, i, trace) {
        let x = lua.create_table()?;
        x.set("input", name(p.input.kind))?;
        x.set("total", ms(p.total()))?;
        let legs = lua.create_table()?;
        for (what, us) in p.legs() {
            let l = lua.create_table()?;
            l.set("what", what)?;
            l.set("ms", ms(us))?;
            legs.push(l)?;
        }
        x.set("legs", legs)?;
        t.set("path", x)?;
    }
    let end = f.end_or_begin() + 1;
    let fresh = lua.create_table()?;
    for p in tl.placements_in(f.begin, end) {
        if name(p.answer) == analysis::REPLAYED {
            continue;
        }
        let x = lua.create_table()?;
        x.set("component", name(p.component))?;
        x.set("answer", name(p.answer))?;
        x.set("why", name(p.why))?;
        x.set("node", hex(p.node))?;
        fresh.push(x)?;
    }
    t.set("fresh", fresh)?;
    t.set(
        "notes",
        lua.create_sequence_from(tl.notes_in(f.gap_from, end).iter().map(|n| n.1.as_str()))?,
    )?;
    let counters = lua.create_table()?;
    for (n, v) in &f.counters {
        counters.set(name(*n), *v)?;
    }
    t.set("counters", counters)?;
    Ok(t)
}

/// A recording's readings over frames `first..=last`: the Perf tab's
/// tables.
fn stats(lua: &Lua, r: &Recording, first: usize, last: usize) -> mlua::Result<Table> {
    let full = r.stream.timeline();
    let sliced =
        (first > 0 || last + 1 < full.frames.len()).then(|| r.trace().slice(full, first, last));
    let trace = sliced.as_ref().unwrap_or(r.trace());
    let owned;
    let tl = if sliced.is_some() {
        owned = trace.timeline();
        &owned
    } else {
        full
    };
    let name = |id| trace.name(id);
    let origin = |o: Origin| match o {
        Origin::Kui => "kui",
        Origin::Host => "host",
        Origin::Extension(ns) => trace.name(ns),
    };
    let t = lua.create_table()?;
    let works: Vec<u64> = tl.frames.iter().map(|f| f.work_us()).collect();
    let (avg, p95, max) = analysis::spread(&works);
    let summary = lua.create_table()?;
    summary.set("frames", tl.frames.len())?;
    summary.set("seconds", ms(tl.end.saturating_sub(tl.start)) / 1000.0)?;
    summary.set("work_avg", avg / 1000.0)?;
    summary.set("work_p95", ms(p95))?;
    summary.set("work_max", ms(max))?;
    summary.set(
        "over_budget",
        works.iter().filter(|&&w| w > r.budget).count(),
    )?;
    summary.set(
        "unchanged",
        tl.frames
            .iter()
            .filter(|f| f.unchanged == Some(true))
            .count(),
    )?;
    summary.set("budget", ms(r.budget))?;
    t.set("summary", summary)?;
    let spans = lua.create_table()?;
    for s in analysis::by_span(tl, &tl.spans).iter().take(40) {
        let x = lua.create_table()?;
        x.set("name", name(s.name))?;
        x.set("origin", origin(s.origin))?;
        x.set("calls", s.calls)?;
        x.set("total", ms(s.total))?;
        x.set("self", ms(s.self_total))?;
        x.set("worst", ms(s.worst))?;
        x.set("cpu", s.cpu.map(ms))?;
        spans.push(x)?;
    }
    t.set("spans", spans)?;
    let origins = lua.create_table()?;
    for (o, us, n) in analysis::by_origin(tl, &tl.spans) {
        let x = lua.create_table()?;
        x.set("origin", origin(o))?;
        x.set("self", ms(us))?;
        x.set("spans", n)?;
        origins.push(x)?;
    }
    t.set("origins", origins)?;
    let components = lua.create_table()?;
    for p in analysis::placements(tl, trace) {
        let x = lua.create_table()?;
        x.set("component", name(p.component))?;
        x.set("placed", p.placed)?;
        x.set("replayed", p.replayed)?;
        let fresh = lua.create_table()?;
        for (a, w, n) in &p.fresh {
            let y = lua.create_table()?;
            y.set("answer", name(*a))?;
            y.set("why", name(*w))?;
            y.set("count", *n)?;
            fresh.push(y)?;
        }
        x.set("fresh", fresh)?;
        components.push(x)?;
    }
    t.set("components", components)?;
    let counters = lua.create_table()?;
    for (n, avg, max) in analysis::counters(tl) {
        let x = lua.create_table()?;
        x.set("name", name(n))?;
        x.set("avg", avg)?;
        x.set("max", max)?;
        counters.push(x)?;
    }
    t.set("counters", counters)?;
    let gauges = lua.create_table()?;
    for (n, last, lo, hi) in analysis::gauges(tl) {
        let x = lua.create_table()?;
        x.set("name", name(n))?;
        x.set("last", last)?;
        x.set("least", lo)?;
        x.set("most", hi)?;
        gauges.push(x)?;
    }
    t.set("gauges", gauges)?;
    let burns = lua.create_table()?;
    for b in analysis::burns(tl, trace) {
        let x = lua.create_table()?;
        x.set("first", b.first + first)?;
        x.set("last", b.last + first)?;
        x.set("frames", b.len())?;
        burns.push(x)?;
    }
    t.set("burns", burns)?;
    let g = analysis::gaps(tl, trace);
    let gaps = lua.create_table()?;
    gaps.set("total", ms(g.total))?;
    gaps.set("handlers", ms(g.handlers))?;
    gaps.set("asleep", ms(g.asleep))?;
    gaps.set("busy", ms(g.busy))?;
    t.set("gaps", gaps)?;
    Ok(t)
}

/// A counter's per-frame totals, or a gauge's readings, as a channel of
/// `at` (ms) and `value`.
fn series(r: &Recording, name: &str) -> Option<Channel> {
    let tl = r.stream.timeline();
    let trace = r.trace();
    let (mut at, mut value) = (Vec::new(), Vec::new());
    for f in &tl.frames {
        for (n, v) in &f.counters {
            if trace.name(*n) == name {
                at.push(ms(f.begin));
                value.push(*v as f64);
            }
        }
    }
    if at.is_empty() {
        for g in &tl.gauges {
            if trace.name(g.name) == name {
                at.push(ms(g.t));
                value.push(g.value);
            }
        }
    }
    if at.is_empty() {
        return None;
    }
    Channel::table(vec![
        ("at".into(), Column::F64(at)),
        ("value".into(), Column::F64(value)),
    ])
    .ok()
}

fn series_names(r: &Recording) -> Vec<String> {
    let tl = r.stream.timeline();
    let trace = r.trace();
    let mut out: Vec<String> = Vec::new();
    for f in &tl.frames {
        for (n, _) in &f.counters {
            let s = trace.name(*n);
            if !out.iter().any(|o| o == s) {
                out.push(s.to_string());
            }
        }
    }
    for g in &tl.gauges {
        let s = trace.name(g.name);
        if !out.iter().any(|o| o == s) {
            out.push(s.to_string());
        }
    }
    out
}

impl UserData for Recording {
    fn add_fields<F: UserDataFields<Self>>(f: &mut F) {
        f.add_field_method_get("spans", |_, r| Ok(LuaChannel(r.spans_view.shown.clone())));
        f.add_field_method_get("frames", |_, r| Ok(LuaChannel(r.frames_view.shown.clone())));
        f.add_field_method_get("instants", |_, r| Ok(LuaChannel(r.instants.clone())));
        f.add_field_method_get("moved", |_, r| Ok(r.moved.load(Ordering::Relaxed)));
        f.add_field_method_get("ended", |_, r| Ok(r.ended));
        f.add_field_method_get("budget", |_, r| Ok(ms(r.budget)));
        f.add_field_method_get("filtered", |_, r| Ok(!r.filter.is_empty()));
        // Why a `where` stopped keeping what came on a poll, if it threw.
        f.add_field_method_get("filter_error", |_, r| Ok(r.filter_error.clone()));
    }

    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_method_mut("poll", |_, r, ()| {
            let (state, n) = r.poll_now().map_err(bad)?;
            Ok((state, n))
        });
        m.add_method("session", |lua, r, ()| {
            let t = lua.create_table()?;
            if let Some(s) = &r.trace().session {
                t.set("kui", s.kui.as_str())?;
                t.set("app", s.app.as_str())?;
                t.set("platform", s.platform.as_str())?;
                t.set("hz", s.refresh_hz)?;
                t.set("budget", s.budget_ms)?;
                t.set("scale", s.scale)?;
                t.set("wall_ms", s.wall_ms)?;
            }
            let tl = r.stream.timeline();
            t.set("start", ms(tl.start))?;
            t.set("finish", ms(tl.end))?;
            t.set("frames", tl.frames.len())?;
            t.set("spans", r.rows.len())?;
            Ok(t)
        });
        m.add_method("frame", |lua, r, row: usize| {
            let Some(i) = r
                .frames_view
                .source_row(row)
                .and_then(|i| r.frame_index.get(i).copied())
            else {
                return Ok(Value::Nil);
            };
            frame_detail(lua, r, i).map(Value::Table)
        });
        m.add_method("span", |lua, r, row: usize| {
            let Some(i) = r.spans_view.source_row(row) else {
                return Ok(Value::Nil);
            };
            let Some(row) = r.rows.get(i) else {
                return Ok(Value::Nil);
            };
            let s = &row.span;
            let t = lua.create_table()?;
            t.set("name", r.trace().name(s.name))?;
            t.set("origin", r.origin_name(s.origin))?;
            t.set("start", ms(s.start))?;
            t.set("len", ms(s.dur))?;
            t.set("self", ms(r.stream.timeline().self_us(s)))?;
            t.set("cpu", s.cpu.map(ms))?;
            t.set("node", hex(s.node))?;
            t.set("cut", s.cut)?;
            t.set("track", r.trace().track_name(s.track))?;
            t.set("depth", row.depth)?;
            t.set("frame", row.frame)?;
            Ok(Value::Table(t))
        });
        m.add_method(
            "stats",
            |lua, r, (first, last): (Option<usize>, Option<usize>)| {
                let n = r.stream.timeline().frames.len();
                if n == 0 {
                    return stats(lua, r, 0, 0);
                }
                let first = first.unwrap_or(0).min(n - 1);
                let last = last.unwrap_or(n - 1).clamp(first, n - 1);
                stats(lua, r, first, last)
            },
        );
        m.add_method("series", |_, r, name: String| {
            Ok(series(r, &name).map(LuaChannel))
        });
        m.add_method("series_names", |lua, r, ()| {
            lua.create_sequence_from(series_names(r))
        });
        m.add_method_mut("filter", |lua, r, spec: Option<Table>| {
            let f = match spec {
                Some(t) => filter_of(lua, &t)?,
                None => Filter::default(),
            };
            r.set_filter(f)?;
            Ok((r.spans_view.shown.len(), r.frames_view.shown.len()))
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::Synth;

    fn state() -> Lua {
        let lua = Lua::new();
        kui_channel::lua::register(&lua).unwrap();
        lua.globals()
            .set("trace", register(&lua, None).unwrap())
            .unwrap();
        lua
    }

    /// A recording read whole: its channels in Kit's waterfall's shape, a
    /// frame's detail, the tables, a series, and a filter's view.
    #[test]
    fn a_script_reads_a_recording_through_channels() {
        let bytes = Synth {
            frames: 400,
            ..Default::default()
        }
        .trace()
        .to_bytes();
        let lua = state();
        lua.globals()
            .set("bytes", lua.create_string(&bytes).unwrap())
            .unwrap();
        lua.load(
            r#"
            local rec = trace.read(bytes)
            frames = #rec.frames
            spans = #rec.spans
            names = table.concat(rec.spans.names, ",")
            local depths, kids = {}, 0
            for i = 0, #rec.spans - 1 do
              local d = rec.spans:get(i, "depth")
              depths[d] = true
              if d > 0 then kids = kids + 1 end
            end
            nested = kids
            build_depth = nil
            for i = 0, #rec.spans - 1 do
              if rec.spans:get(i, "name") == "build" then build_depth = rec.spans:get(i, "depth") break end
            end
            local f = rec:frame(10)
            verdict = f.verdict.kind
            has_phases = f.phases.build > 0
            local s = rec:stats()
            top = s.spans[1].name
            components = #s.components
            burns = #s.burns
            local sub = rec:stats(10, 19)
            sub_frames = sub.summary.frames
            series = #rec:series("rows")
            instants = #rec.instants
            local nspans, nframes = rec:filter { origins = { "host" }, min_us = 1 }
            filtered = nspans
            host_only = true
            for i = 0, #rec.spans - 1 do
              if rec.spans:get(i, "origin") ~= "host" then host_only = false end
            end
            local _, slow = rec:filter { verdicts = { "slow" } }
            slow_frames = slow
            slow_first = rec:frame(0) and rec:frame(0).verdict.kind
            local all = rec:filter()
            unfiltered = all
            "#,
        )
        .exec()
        .unwrap();
        let g = lua.globals();
        assert_eq!(g.get::<i64>("frames").unwrap(), 400);
        let spans = g.get::<i64>("spans").unwrap();
        assert!(spans > 4000, "{spans}");
        assert!(
            g.get::<String>("names")
                .unwrap()
                .starts_with("name,start,len,depth,group,target")
        );
        assert!(g.get::<i64>("nested").unwrap() > 1000);
        assert_eq!(g.get::<i64>("build_depth").unwrap(), 0);
        assert!(g.get::<bool>("has_phases").unwrap());
        assert_eq!(g.get::<String>("top").unwrap(), "build");
        assert!(g.get::<i64>("components").unwrap() > 0);
        assert_eq!(g.get::<i64>("sub_frames").unwrap(), 10);
        assert!(g.get::<i64>("series").unwrap() > 300);
        assert!(g.get::<i64>("instants").unwrap() > 100);
        let filtered = g.get::<i64>("filtered").unwrap();
        assert!(filtered > 0 && filtered < spans);
        assert!(g.get::<bool>("host_only").unwrap());
        let slow = g.get::<i64>("slow_frames").unwrap();
        assert!(slow < 400);
        if slow > 0 {
            assert_eq!(g.get::<String>("slow_first").unwrap(), "slow");
        }
        assert_eq!(g.get::<i64>("unfiltered").unwrap(), spans);
        let _ = g.get::<Value>("verdict").unwrap();
    }

    /// A file still being written: what came is appended, the views' handles
    /// stay, and a file written again from its start begins anew.
    #[test]
    fn a_watched_file_grows_the_channels_and_starts_again() {
        use std::io::Write;
        let path =
            std::env::temp_dir().join(format!("kui-trace-lua-{}.kuitrace", std::process::id()));
        let bytes = Synth {
            frames: 200,
            ..Default::default()
        }
        .trace()
        .to_bytes();
        std::fs::write(&path, &bytes[..bytes.len() / 2]).unwrap();
        let lua = state();
        lua.globals().set("path", path.to_str().unwrap()).unwrap();
        lua.load(
            r#"
            rec = trace.open(path)
            first = #rec.frames
            spans_id = rec.spans.id
            idle = rec:poll()
            "#,
        )
        .exec()
        .unwrap();
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(&bytes[bytes.len() / 2..])
            .unwrap();
        lua.load(
            r#"
            more = rec:poll()
            second = #rec.frames
            same_id = rec.spans.id == spans_id
            "#,
        )
        .exec()
        .unwrap();
        let small = Synth {
            frames: 20,
            seed: 9,
            ..Default::default()
        }
        .trace()
        .to_bytes();
        std::fs::write(&path, &small).unwrap();
        lua.load(
            r#"
            reset = rec:poll()
            third = #rec.frames
            kept_id = rec.spans.id == spans_id
            "#,
        )
        .exec()
        .unwrap();
        let g = lua.globals();
        let first = g.get::<i64>("first").unwrap();
        assert!(first > 50 && first < 200, "{first}");
        assert_eq!(g.get::<String>("idle").unwrap(), "idle");
        assert_eq!(g.get::<String>("more").unwrap(), "more");
        assert_eq!(
            g.get::<i64>("second").unwrap(),
            199,
            "the last frame waits for the next"
        );
        assert!(g.get::<bool>("same_id").unwrap());
        assert_eq!(g.get::<String>("reset").unwrap(), "reset");
        assert_eq!(g.get::<i64>("third").unwrap(), 19);
        assert!(g.get::<bool>("kept_id").unwrap());
        let _ = std::fs::remove_file(&path);
    }
}

#[cfg(test)]
mod timing {
    /// `KUI_TRACE_BIG=path cargo test --release -p kui-trace --features lua
    /// opening_a_long_recording -- --ignored --nocapture`: how long a
    /// recording takes from its file to channels (ADR 0075's open budget).
    #[test]
    #[ignore]
    fn opening_a_long_recording() {
        let Ok(path) = std::env::var("KUI_TRACE_BIG") else {
            return;
        };
        let lua = mlua::Lua::new();
        lua.globals()
            .set("trace", super::register(&lua, None).unwrap())
            .unwrap();
        lua.globals().set("path", path).unwrap();
        let t = std::time::Instant::now();
        lua.load("rec = trace.open(path, { watch = false }) n = #rec.spans f = #rec.frames")
            .exec()
            .unwrap();
        let open = t.elapsed();
        let t = std::time::Instant::now();
        lua.load("rec:filter { name = 'rows' }").exec().unwrap();
        let filter = t.elapsed();
        let t = std::time::Instant::now();
        lua.load(
            "kept = rec:filter { where = function(s) return s.len > 0.5 and s.name:find('fill', 1, true) ~= nil end }",
        )
        .exec()
        .unwrap();
        let predicate = t.elapsed();
        let g = lua.globals();
        println!(
            "open {:?} ({} spans, {} frames), filter {:?}, a Lua where over every span {:?} ({} kept)",
            open,
            g.get::<i64>("n").unwrap(),
            g.get::<i64>("f").unwrap(),
            filter,
            predicate,
            g.get::<i64>("kept").unwrap()
        );
    }
}
