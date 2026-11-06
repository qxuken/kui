//! A recording for a Lua script, with the `lua` feature (backlog F251, ADR
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
//!   Unavailable numeric work/phase measurements are NaN; query/detail
//!   tables use nil instead. A present is required for total work/verdicts.
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

use crate::analysis;
use crate::query::{self, Explorer, Operation, Query, Selection};
use crate::query::{origin_name as origin_of, placement_of, verdict_kind};
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
    /// The placement a `fill …` span drew — its component, its answer
    /// (replayed, or fresh and how) and why — what the waterfall and the
    /// name filter call it, where every component's fill read `fill kit`.
    component: Option<[NameId; 3]>,
}

/// What the views keep (decision 8): nothing filtered is everything.
#[derive(Clone, Debug, Default)]
struct Filter {
    native: query::Filter,
    predicate: Option<Predicate>,
}

impl std::ops::Deref for Filter {
    type Target = query::Filter;
    fn deref(&self) -> &Self::Target {
        &self.native
    }
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
        self.predicate.is_none() && self.native == query::Filter::default()
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
    query_index: std::sync::OnceLock<query::Index>,
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
    /// Each placed frame's `burn` column, kept to notice a burn that grew
    /// back over frames placed before it was one (backlog RG171).
    frame_burn: Vec<bool>,
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
        ("id".into(), Column::I64(Vec::new())),
        ("parent".into(), Column::I64(Vec::new())),
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
            query_index: std::sync::OnceLock::new(),
            transport,
            _watch: watch,
            moved,
            ended: false,
            budget: 0,
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
            frame_burn: Vec::new(),
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
        // The `where`'s strings were the old recording's names, by its
        // name table's numbers: the new one numbers its own.
        if let Some(p) = &self.filter.predicate {
            p.strings.borrow_mut().clear();
        }
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
                component: None,
            });
        }
    }

    /// Takes in the records the stream read since the last time: spans
    /// whose parents have come, the instants, the frames finished; and the
    /// channels and views grown with them.
    fn absorb(&mut self) {
        let budget = query::budget_us(self.trace()).unwrap_or(0);
        let changed = self.seen != self.trace().records.len() || self.budget != budget;
        if changed {
            self.query_index.take();
        }
        self.budget = budget;
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
            row.frame =
                query::frame_for_time(tl, s.start).map_or(tl.frames.len() as i64, |i| i as i64);
        }
        for row in &mut placed {
            row.self_us = tl.self_us(&row.span);
        }
        let trace = self.stream.trace();
        for row in &mut placed {
            row.component =
                placement_of(tl, trace, &row.span).map(|p| [p.component, p.answer, p.why]);
        }
        for row in &placed {
            let self_us = row.self_us;
            let s = &row.span;
            let group = group(&mut self.groups, s.origin);
            let name = trace.name(row.component.map_or(s.name, |c| c[0]));
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
            cols.i(10, s.id as i64);
            cols.i(11, s.parent as i64);
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
        // Frames finished: every one before the last; a completed present
        // seals the last too, including a saved file still being watched.
        let tl = self.stream.timeline();
        let trace = self.stream.trace();
        let done = if self.ended || tl.frames.last().is_some_and(|f| f.present.is_some()) {
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
                let v = (self.budget > 0 && query::frame_work_us(f).is_some())
                    .then(|| analysis::verdict(tl, i, self.budget, &burns, trace));
                let kind = v.as_ref().map_or("unavailable", verdict_kind);
                fc.i(0, f.frame as i64);
                fc.i(1, i as i64);
                fc.f(2, ms(f.begin));
                fc.f(3, ms(f.end_or_begin()));
                // Numeric channels use NaN for missing measurements; plots
                // leave gaps and channel sorting puts these cells last.
                fc.f(4, query::frame_work_us(f).map_or(f64::NAN, ms));
                for (k, (name, us)) in p.named().iter().enumerate() {
                    let known = f.end.is_some()
                        && (!matches!(*name, "render" | "vsync wait") || f.present.is_some());
                    fc.f(5 + k, if known { ms(*us) } else { f64::NAN });
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
                self.frame_burn.push(in_burn(i));
            }
            self.frames = self
                .frames
                .append(fc.0, None)
                .expect("the frames' own columns");
            // A burn is a run of BURN_FRAMES, so the frames of one that
            // came before it was that long were placed as no burn; they
            // are one now, and their verdict may be too (backlog RG171).
            // Written once a burn, when it reaches the frames placed.
            let mut regraded = false;
            for b in &burns {
                for i in b.first..(b.last + 1).min(first_new_frame) {
                    if !self.frame_burn[i] {
                        self.frame_burn[i] = true;
                        self.frame_verdict[i] =
                            if self.budget > 0 && query::frame_work_us(&tl.frames[i]).is_some() {
                                verdict_kind(&analysis::verdict(tl, i, self.budget, &burns, trace))
                            } else {
                                "unavailable"
                            };
                        regraded = true;
                    }
                }
            }
            if regraded {
                self.regrade_frames();
                if self.filter.verdicts.is_some()
                    || self.filter.hide_unchanged
                    || self.filter.predicate.is_some()
                {
                    if let Err(e) = self.set_filter(self.filter.clone()) {
                        self.filter_error = Some(e.to_string());
                    }
                } else {
                    self.grow_views(first_new_span, self.frame_index.len());
                    self.refilter_frames();
                }
                return;
            }
        }
        // Gap work can precede its frame in the file. Frame-dependent filters
        // must reconsider those existing spans when more recording data comes;
        // simply appending newly read spans would leave the timeline stale.
        if changed
            && (self.filter.verdicts.is_some()
                || self.filter.hide_unchanged
                || self.filter.predicate.is_some())
        {
            if let Err(e) = self.set_filter(self.filter.clone()) {
                self.filter_error = Some(e.to_string());
            }
        } else {
            self.grow_views(first_new_span, first_new_frame);
        }
    }

    /// The frames' next revision with every frame's verdict and burn as
    /// they stand: the rest of their columns as they were.
    fn regrade_frames(&mut self) {
        let names = self.frames.names().to_vec();
        let all: Vec<usize> = (0..self.frames.len()).collect();
        let mut cols = self.frames.take_columns(&all).expect("rows it holds");
        for (name, col) in &mut cols {
            match name.as_str() {
                "verdict" => *col = Column::strs(&self.frame_verdict),
                "burn" => *col = Column::Bool(self.frame_burn.clone()),
                _ => {}
            }
        }
        debug_assert_eq!(
            cols.iter().map(|(n, _)| n).collect::<Vec<_>>(),
            names.iter().collect::<Vec<_>>()
        );
        self.frames = self.frames.publish(cols).expect("the frames' own columns");
    }

    /// The frames' view shown again from the start: after frames were
    /// regraded, a verdict filter keeps another set.
    fn refilter_frames(&mut self) {
        if self.filter.is_empty() {
            self.frames_view.shown = self.frames_view.shown.publish_from(&self.frames);
            self.frames_view.rows = None;
            return;
        }
        let frames: Vec<usize> = (0..self.frame_index.len())
            .filter(|&i| self.keeps_frame(i))
            .collect();
        let cols = self.frames.take_columns(&frames).expect("rows it holds");
        self.frames_view.shown = self.frames_view.shown.publish(cols).expect("its columns");
        self.frames_view.rows = Some(frames);
    }

    fn keeps_span(&self, i: usize) -> mlua::Result<bool> {
        let f = &self.filter;
        let row = &self.rows[i];
        let s = &row.span;
        if !query::matches_span(self.trace(), self.stream.timeline(), s, &f.native) {
            return Ok(false);
        }
        if f.verdicts.is_some() || f.hide_unchanged {
            let tl = self.stream.timeline();
            let Some(i) = query::frame_for_time(tl, s.start) else {
                return Ok(false);
            };
            let frame = tl.frames.get(i);
            let verdict = self
                .frame_verdict
                .get(i)
                .copied()
                .filter(|v| *v != "unavailable");
            if !query::matches_frame(frame.and_then(|f| f.unchanged), verdict, &f.native) {
                return Ok(false);
            }
        }
        let trace = self.trace();
        if let Some(p) = &f.predicate {
            let t = &p.span;
            let k = &p.keys;
            // Names by their ids from 2; kui's and the app's origins at 0
            // and 1.
            // A component's fill by its component, as the waterfall names it.
            let shown = row.component.map_or(s.name, |c| c[0]);
            t.raw_set(&k[0], p.string(shown as usize + 2, trace.name(shown))?)?;
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
            t.raw_set(
                &k[7],
                query::frame_for_time(self.stream.timeline(), s.start).map_or(-1, |i| i as i64),
            )?;
            return p.test.call::<bool>(t);
        }
        Ok(true)
    }

    fn keeps_frame(&self, i: usize) -> bool {
        query::matches_frame(
            Some(self.frame_unchanged[i]),
            Some(self.frame_verdict[i]).filter(|v| *v != "unavailable"),
            &self.filter.native,
        )
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

    fn explorer(&self) -> Explorer<'_> {
        let index = self
            .query_index
            .get_or_init(|| Explorer::new(self.trace(), self.stream.timeline()).into_index());
        Explorer::with_index(self.trace(), self.stream.timeline(), index)
    }

    fn query_filter(&self) -> mlua::Result<query::Filter> {
        let mut f = self.filter.native.clone();
        if self.filter.predicate.is_some() {
            let mut ids = Vec::new();
            for i in 0..self.rows.len() {
                if self.keeps_span(i)? {
                    ids.push(self.rows[i].span.id);
                }
            }
            f.span_ids = Some(ids);
        }
        Ok(f)
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
        native: query::Filter {
            origins: list("origins")?,
            name: t.get::<Option<String>>("name")?.filter(|n| !n.is_empty()),
            hide_names: list("hide_names")?.unwrap_or_default(),
            min_us: t.get::<Option<f64>>("min_us")?.unwrap_or(0.0).max(0.0) as u64,
            verdicts: list("verdicts")?,
            hide_unchanged: t.get::<Option<bool>>("hide_unchanged")?.unwrap_or(false),
            span_ids: None,
        },
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

fn table_of(lua: &Lua, value: &kui_value::Value) -> mlua::Result<Table> {
    match kui_value::lua::to_lua(lua, value)? {
        Value::Table(t) => Ok(t),
        _ => unreachable!("query presentation is a table"),
    }
}

fn frame_detail(lua: &Lua, r: &Recording, i: usize) -> mlua::Result<Table> {
    let ex = r.explorer();
    table_of(
        lua,
        &crate::query_value::frame(r.trace(), &ex.frame_detail(i)),
    )
}

fn span_detail(lua: &Lua, r: &Recording, id: u32) -> mlua::Result<Value> {
    let ex = r.explorer();
    match ex.span_detail(id) {
        Some(d) => table_of(lua, &crate::query_value::span(r.trace(), &d)).map(Value::Table),
        None => Ok(Value::Nil),
    }
}

fn stats_empty(lua: &Lua, r: &Recording) -> mlua::Result<Table> {
    let ex = r.explorer();
    let filter = r.query_filter()?;
    let scope = query::Scope {
        requested: Selection::Whole,
        indices: Vec::new(),
        intervals: Vec::new(),
    };
    table_of(
        lua,
        &crate::query_value::readings(
            r.trace(),
            r.stream.timeline(),
            &scope,
            &filter,
            &ex.readings(&scope, &filter),
        ),
    )
}

fn stats(lua: &Lua, r: &Recording, selection: Selection) -> mlua::Result<Table> {
    let tl = r.stream.timeline();
    let ex = r.explorer();
    let filter = r.query_filter()?;
    let scope = ex.resolve(&selection, &filter).map_err(bad)?;
    let readings = ex.readings(&scope, &filter);
    table_of(
        lua,
        &crate::query_value::readings(r.trace(), tl, &scope, &filter, &readings),
    )
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
        f.add_field_method_get("budget", |_, r| Ok((r.budget > 0).then(|| ms(r.budget))));
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
            let origins: std::collections::BTreeSet<_> = tl
                .spans
                .iter()
                .map(|s| query::origin_name(r.trace(), s.origin))
                .collect();
            t.set("origins", lua.create_sequence_from(origins)?)?;
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
            let Some(row) = r.spans_view.source_row(row).and_then(|i| r.rows.get(i)) else {
                return Ok(Value::Nil);
            };
            span_detail(lua, r, row.span.id)
        });
        m.add_method("frame_id", |lua, r, id: u64| {
            match r.explorer().frame_index(id) {
                Some(i) => frame_detail(lua, r, i).map(Value::Table),
                None => Ok(Value::Nil),
            }
        });
        m.add_method("span_id", |lua, r, id: u32| span_detail(lua, r, id));
        m.add_method(
            "stats",
            |lua, r, (first, last): (Option<usize>, Option<usize>)| {
                let frames = &r.stream.timeline().frames;
                let selection = if first.is_none() && last.is_none() {
                    Selection::Whole
                } else {
                    let a = first.unwrap_or(0);
                    let b = last.unwrap_or(frames.len().saturating_sub(1));
                    match (frames.get(a), frames.get(b)) {
                        (Some(f), Some(l)) if a <= b => Selection::Frames {
                            first: f.frame,
                            last: l.frame,
                        },
                        _ => return stats_empty(lua, r),
                    }
                };
                stats(lua, r, selection)
            },
        );
        m.add_method("readings", |lua, r, (from, to): (f64, f64)| {
            stats(
                lua,
                r,
                Selection::Time {
                    from_us: query::milliseconds(from).map_err(bad)?,
                    to_us: query::milliseconds(to).map_err(bad)?,
                },
            )
        });
        m.add_method("scope_readings", |lua, r, s: Table| {
            let selection = match s.get::<String>("kind")?.as_str() {
                "whole" => Selection::Whole,
                "frames" => Selection::Frames {
                    first: s.get("first")?,
                    last: s.get("last")?,
                },
                "time" => Selection::Time {
                    from_us: query::milliseconds(s.get("from")?).map_err(bad)?,
                    to_us: query::milliseconds(s.get("to")?).map_err(bad)?,
                },
                _ => return Err(bad("unknown measurement scope")),
            };
            stats(lua, r, selection)
        });
        m.add_method("focus", |lua, r, (flag, value): (String, String)| {
            let ex = r.explorer();
            let mut frame = None;
            let mut span = None;
            let selection = match flag.as_str() {
                "--frame" => {
                    let id = value.parse::<u64>().map_err(bad)?;
                    if ex.frame_index(id).is_none() {
                        return Err(bad(format!("no frame {id} in this recording")));
                    }
                    frame = Some(id);
                    Selection::Frames {
                        first: id,
                        last: id,
                    }
                }
                "--span" => {
                    let id = value.parse::<u32>().map_err(bad)?;
                    let detail = ex
                        .span_detail(id)
                        .ok_or_else(|| bad(format!("no span {id} in this recording")))?;
                    span = Some(id);
                    detail
                        .frame
                        .map_or(Selection::Whole, |id| Selection::Frames {
                            first: id,
                            last: id,
                        })
                }
                "--frames" => query::frame_range(&value).map_err(bad)?,
                "--time" => query::time_range(&value).map_err(bad)?,
                _ => return Err(bad("unknown focus selector")),
            };
            let scope = ex
                .resolve(&selection, &query::Filter::default())
                .map_err(bad)?;
            if scope.intervals.is_empty() {
                return Err(bad("focus selection contains no recorded frames"));
            }
            let t = table_of(lua, &crate::query_value::scope(r.stream.timeline(), &scope))?;
            t.set("frame_id", frame)?;
            t.set("span_id", span)?;
            if let Some(id) = span
                && let Some(d) = ex.span_detail(id)
            {
                t.set("span_start", ms(d.span.start))?;
                t.set("span_finish", ms(d.span.end()))?;
            }
            Ok(t)
        });
        m.add_method(
            "query",
            |lua, r, (operation, opts): (String, Option<Table>)| {
                let opts = opts.unwrap_or(lua.create_table()?);
                let op = match operation.as_str() {
                    "summary" => Operation::Summary,
                    "frames" => Operation::Frames,
                    "spans" => Operation::Spans,
                    "counters" => Operation::Counters,
                    "marks" => Operation::Marks,
                    "frame" => Operation::Frame(opts.get("id")?),
                    "span" => Operation::Span(opts.get("id")?),
                    _ => return Err(bad("unknown perf operation")),
                };
                let mut q = Query::new(op);
                q.filter = r.query_filter()?;
                for (first, last) in [("first_frame", "last_frame"), ("from_ms", "to_ms")] {
                    if opts.contains_key(first)? != opts.contains_key(last)? {
                        return Err(bad(format!("{first} and {last} must be provided together")));
                    }
                }
                if opts.get::<Option<bool>>("all")?.unwrap_or(false)
                    && opts.contains_key("limit")?
                {
                    return Err(bad("choose limit or all"));
                }
                if let Some(first) = opts.get::<Option<u64>>("first_frame")? {
                    q.selection = Selection::Frames {
                        first,
                        last: opts.get("last_frame")?,
                    };
                }
                if let Some(from) = opts.get::<Option<f64>>("from_ms")? {
                    if q.selection != Selection::Whole {
                        return Err(bad("choose frames or time, not both"));
                    }
                    q.selection = Selection::Time {
                        from_us: query::milliseconds(from).map_err(bad)?,
                        to_us: query::milliseconds(opts.get("to_ms")?).map_err(bad)?,
                    };
                }
                if let Some(sort) = opts.get::<Option<String>>("sort")? {
                    q.set_sort(sort);
                }
                if let Some(desc) = opts.get::<Option<bool>>("descending")? {
                    q.descending = desc;
                }
                if opts.get::<Option<bool>>("all")?.unwrap_or(false) {
                    q.limit = None;
                } else if let Some(limit) = opts.get::<Option<usize>>("limit")? {
                    q.limit = Some(limit);
                }
                let ex = r.explorer();
                let result = ex.execute(&q).map_err(bad)?;
                table_of(
                    lua,
                    &crate::query_value::result(r.trace(), r.stream.timeline(), &result, ""),
                )
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

    #[test]
    fn frame_boundary_span_has_one_owner_in_channels_queries_and_details() {
        let mut trace = Trace::new();
        trace.session = Some(crate::Session {
            budget_ms: 10.0,
            ..Default::default()
        });
        let name = trace.names.get("boundary work").0;
        for (frame, begin, end, unchanged) in [(10, 0, 1000, true), (20, 1000, 2000, false)] {
            trace.records.extend([
                Record::FrameBegin {
                    t: begin,
                    frame,
                    causes: vec![],
                },
                Record::FrameEnd {
                    t: end,
                    frame,
                    unchanged: Some(unchanged),
                    nodes: 0,
                },
                Record::Present {
                    t: end,
                    frame,
                    render: 0,
                    wait: 0,
                },
            ]);
        }
        trace.records.push(Record::Span(Span {
            track: crate::Track::MAIN,
            id: 1,
            parent: 0,
            name,
            origin: Origin::Host,
            start: 1000,
            dur: 100,
            cpu: None,
            node: 0,
            cut: false,
        }));
        let lua = state();
        lua.globals()
            .set("bytes", lua.create_string(trace.to_bytes()).unwrap())
            .unwrap();
        lua.load(
            r#"
            local rec = trace.read(bytes)
            assert(rec:span_id(1).frame == 20, "boundary span belongs to the following frame")
            assert(rec.spans:get(0, "frame") == 1, "channel frame index identifies the same owner")
            rec:filter { hide_unchanged = true }
            assert(#rec.frames == 1 and rec.frames:get(0, "frame") == 20)
            assert(#rec.spans == 1, "the changed frame's boundary span stays in the timeline")
            local q = rec:query("spans", { all = true })
            assert(q.matched == 1 and q.data[1].example_id == 1)
            assert(rec:stats().spans[1].example_id == 1)
            rec:filter { verdicts = { "wasted" } }
            assert(#rec.frames == 1 and rec.frames:get(0, "frame") == 10)
            assert(#rec.spans == 0 and rec:query("spans", { all = true }).matched == 0,
                   "excluding the owner excludes the span in both interfaces")
        "#,
        )
        .exec()
        .unwrap();
    }

    #[test]
    fn frame_filters_reconsider_gap_work_when_the_following_frame_arrives() {
        use std::io::Write;
        let path = std::env::temp_dir().join(format!(
            "kui-trace-frame-membership-{}.kuitrace",
            std::process::id()
        ));
        let mut before = Trace::new();
        let name = before.names.get("background").0;
        before.records.extend([
            Record::FrameBegin {
                t: 0,
                frame: 10,
                causes: vec![],
            },
            Record::FrameEnd {
                t: 1000,
                frame: 10,
                unchanged: Some(true),
                nodes: 0,
            },
            Record::Present {
                t: 2000,
                frame: 10,
                render: 100,
                wait: 900,
            },
            Record::Span(Span {
                track: crate::Track(2),
                id: 1,
                parent: 0,
                name,
                origin: Origin::Host,
                start: 1500,
                dur: 100,
                cpu: None,
                node: 0,
                cut: false,
            }),
        ]);
        let mut after = before.clone();
        after.records.extend([
            Record::FrameBegin {
                t: 3000,
                frame: 20,
                causes: vec![],
            },
            Record::FrameEnd {
                t: 4000,
                frame: 20,
                unchanged: Some(false),
                nodes: 0,
            },
            Record::Present {
                t: 5000,
                frame: 20,
                render: 100,
                wait: 900,
            },
        ]);
        let prefix = before.to_bytes();
        let complete = after.to_bytes();
        assert_eq!(prefix, &complete[..prefix.len()]);
        std::fs::write(&path, &prefix).unwrap();
        let lua = state();
        lua.globals().set("path", path.to_str().unwrap()).unwrap();
        lua.load(
            r#"
            rec=trace.open(path)
            rec:filter{hide_unchanged=true}
            assert(#rec.spans==0 and rec:query("spans").matched==0)
        "#,
        )
        .exec()
        .unwrap();
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(&complete[prefix.len()..])
            .unwrap();
        let result = lua
            .load(
                r#"
            rec:poll()
            assert(rec:span_id(1).frame==20)
            assert(rec:query("spans").matched==1)
            assert(#rec.spans==1,"the timeline must reconsider work read before its frame arrived")
            assert(rec.spans:get(0,"id")==1)
        "#,
            )
            .exec();
        let _ = std::fs::remove_file(path);
        result.unwrap();
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
            -- A component's fill by its component, with how it was drawn.
            fills_named, fills_raw = 0, 0
            for i = 0, #rec.spans - 1 do
              local n = rec.spans:get(i, "name")
              if n:find("^kit/") then fills_named = fills_named + 1 end
              if n == "fill kit" then fills_raw = fills_raw + 1 end
            end
            for i = 0, #rec.spans - 1 do
              if rec.spans:get(i, "name"):find("^kit/") then
                local d = rec:span(i)
                fill_detail = d.component == d.name and d.span == "fill kit" and d.answer ~= nil
                break
              end
            end
            local f = rec:frame(10)
            verdict = f.verdict.kind
            has_phases = f.phases.build > 0
            local s = rec:stats()
            self_sorted = true
            for i = 2, #s.spans do if s.spans[i].self > s.spans[i-1].self then self_sorted = false end end
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
        // Every component's fill by its component: the runtime records
        // each one as `fill kit`, its placement as it ends.
        assert!(g.get::<i64>("fills_named").unwrap() > 400);
        assert_eq!(
            g.get::<i64>("fills_raw").unwrap(),
            0,
            "no fill left unnamed"
        );
        assert!(
            g.get::<bool>("fill_detail").unwrap(),
            "its detail says how it was drawn"
        );
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
        assert!(
            g.get::<bool>("self_sorted").unwrap(),
            "span groups rank by self cost"
        );
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
            cached_first = rec:query("summary").data.frames
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
            cached_second = rec:query("summary").data.frames
            assert(rec:frame_id(200).frame == 200)
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
            cached_third = rec:query("summary").data.frames
            assert(rec:frame_id(200) == nil)
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
            200,
            "a completed present seals the last frame without waiting for a successor"
        );
        assert_eq!(
            g.get::<i64>("cached_second").unwrap(),
            200,
            "appending invalidates the shared query index"
        );
        assert_eq!(
            g.get::<i64>("cached_third").unwrap(),
            20,
            "rewriting invalidates old ID references"
        );
        assert!(g.get::<bool>("same_id").unwrap());
        assert_eq!(g.get::<String>("reset").unwrap(), "reset");
        assert_eq!(g.get::<i64>("third").unwrap(), 20);
        assert!(g.get::<bool>("kept_id").unwrap());
        let _ = std::fs::remove_file(&path);
    }

    /// Grown a piece at a time, a recording ends with the verdicts and
    /// burns it has read whole: a burn longer than a poll's frames marks
    /// the frames placed before it was one (backlog RG171).
    #[test]
    fn a_recording_grown_in_pieces_grades_its_frames_as_read_whole() {
        use std::io::Write;
        let path = std::env::temp_dir().join(format!(
            "kui-trace-lua-burn-{}.kuitrace",
            std::process::id()
        ));
        let bytes = Synth {
            frames: 400,
            ..Default::default()
        }
        .trace()
        .to_bytes();
        let grades = r#"
            local out = {}
            for i = 0, #rec.frames - 1 do
              out[#out + 1] = rec.frames:get(i, "verdict") .. (rec.frames:get(i, "burn") and "*" or "")
            end
            return table.concat(out, " ")
        "#;
        std::fs::write(&path, &bytes).unwrap();
        let lua = state();
        lua.globals().set("path", path.to_str().unwrap()).unwrap();
        lua.load("rec = trace.open(path)").exec().unwrap();
        let whole: String = lua.load(grades).eval().unwrap();
        assert!(whole.contains('*'), "the recording has a burn to grow");
        let pieces = 20;
        let step = bytes.len().div_ceil(pieces);
        std::fs::write(&path, &bytes[..step]).unwrap();
        lua.load("rec = trace.open(path); burned = rec:filter { verdicts = { 'burn' } }")
            .exec()
            .unwrap();
        for k in 1..pieces {
            let (a, z) = (k * step, ((k + 1) * step).min(bytes.len()));
            if a < z {
                std::fs::OpenOptions::new()
                    .append(true)
                    .open(&path)
                    .unwrap()
                    .write_all(&bytes[a..z])
                    .unwrap();
            }
            lua.load("rec:poll()").exec().unwrap();
        }
        // The view a verdict filter keeps, kept through the growth,
        // follows the regrade; then the frames whole.
        let shown: i64 = lua.load("return #rec.frames").eval().unwrap();
        lua.load("rec:filter()").exec().unwrap();
        let grown: String = lua.load(grades).eval().unwrap();
        // Both wait for a frame after the last, the file being watched.
        let (w, g) = (
            whole.split(' ').collect::<Vec<_>>(),
            grown.split(' ').collect::<Vec<_>>(),
        );
        assert_eq!(g.len(), w.len());
        assert_eq!(w, g, "graded as read whole");
        let want = g.iter().filter(|v| v.starts_with("burn")).count() as i64;
        assert!(want > 0);
        assert_eq!(shown, want, "the burn filter's view");
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
