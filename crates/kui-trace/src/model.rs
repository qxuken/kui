//! A recording read whole: its names, its records, and the timeline they
//! make — frames as intervals, each with the gap before it, and every
//! other record sorted by time — which the devtools' Perf tab, `kui-perf`,
//! the adapters and `diff` all read.

use std::collections::HashMap;

use crate::codec::{Encoder, Error, Reader, header};
use crate::record::{LoopEvent, NameId, Record, Session, Span, Track};

/// Names to ids, as a writer hands them out: each string once.
#[derive(Clone, Debug, Default)]
pub struct Names {
    map: HashMap<String, NameId>,
    list: Vec<String>,
}

impl Names {
    pub fn new() -> Self {
        Names {
            map: HashMap::new(),
            list: vec![String::new()],
        }
    }

    /// `s`'s id, and whether it is new — a [`Record::Name`] to write.
    pub fn get(&mut self, s: &str) -> (NameId, bool) {
        if s.is_empty() {
            return (0, false);
        }
        if let Some(&id) = self.map.get(s) {
            return (id, false);
        }
        if self.list.is_empty() {
            self.list.push(String::new());
        }
        let id = self.list.len() as NameId;
        self.list.push(s.to_string());
        self.map.insert(s.to_string(), id);
        (id, true)
    }

    /// The string an id stands for; the empty string for one never given.
    pub fn text(&self, id: NameId) -> &str {
        self.list.get(id as usize).map_or("", String::as_str)
    }

    /// Every name, by id; the first is the empty string.
    pub fn all(&self) -> &[String] {
        &self.list
    }

    fn set(&mut self, id: NameId, text: String) {
        let i = id as usize;
        if self.list.len() <= i {
            self.list.resize(i + 1, String::new());
        }
        self.map.insert(text.clone(), id);
        self.list[i] = text;
    }
}

/// A recording in memory: what it was made of, its names and tracks, and
/// the rest of its records in the order written.
#[derive(Clone, Debug, Default)]
pub struct Trace {
    pub session: Option<Session>,
    pub names: Names,
    pub tracks: Vec<(Track, NameId)>,
    pub records: Vec<Record>,
}

impl Trace {
    pub fn new() -> Self {
        Trace {
            names: Names::new(),
            ..Default::default()
        }
    }

    /// The recording these bytes hold, read up to where they stop.
    pub fn from_bytes(b: &[u8]) -> Result<Trace, Error> {
        let mut t = Trace::new();
        for r in Reader::new(b)? {
            t.push(r);
        }
        Ok(t)
    }

    /// Takes one record as a reader hands it: names and tracks into their
    /// tables, the session aside, the rest kept.
    pub fn push(&mut self, r: Record) {
        match r {
            Record::Session(s) => self.session = Some(s),
            Record::Name { id, text } => self.names.set(id, text),
            Record::Track { id, name } => {
                if !self.tracks.iter().any(|(t, _)| *t == id) {
                    self.tracks.push((id, name));
                }
            }
            other => self.records.push(other),
        }
    }

    /// A name's string.
    pub fn name(&self, id: NameId) -> &str {
        self.names.text(id)
    }

    /// A track's name: `main`, `loop`, or the one declared.
    pub fn track_name(&self, t: Track) -> &str {
        match t {
            Track::MAIN => "main",
            Track::LOOP => "loop",
            t => self
                .tracks
                .iter()
                .find(|(id, _)| *id == t)
                .map_or("", |(_, n)| self.name(*n)),
        }
    }

    /// The recording as bytes: the header, the session, every name and
    /// track, then the records.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = header();
        let mut enc = Encoder::new();
        if let Some(s) = &self.session {
            enc.encode(&Record::Session(s.clone()), &mut out);
        }
        for (id, text) in self.names.all().iter().enumerate().skip(1) {
            // A gap a reader padded with the empty string is not written.
            if text.is_empty() {
                continue;
            }
            enc.encode(
                &Record::Name {
                    id: id as NameId,
                    text: text.clone(),
                },
                &mut out,
            );
        }
        for (id, name) in &self.tracks {
            enc.encode(
                &Record::Track {
                    id: *id,
                    name: *name,
                },
                &mut out,
            );
        }
        for r in &self.records {
            enc.encode(r, &mut out);
        }
        out
    }

    /// The timeline the records make.
    pub fn timeline(&self) -> Timeline {
        Timeline::of(&self.records)
    }
}

/// Who held a frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Holder {
    pub what: NameId,
    pub name: NameId,
    pub node: u64,
}

/// A frame drawn: the driver's render and vsync's wait, µs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Present {
    pub t: u64,
    pub render: u64,
    pub wait: u64,
}

/// One frame of the timeline.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub frame: u64,
    pub begin: u64,
    /// When it finished; `None` for one the recording stopped inside.
    pub end: Option<u64>,
    /// Where its gap starts: the frame before's end, or the recording's
    /// start.
    pub gap_from: u64,
    pub causes: Vec<NameId>,
    pub holders: Vec<Holder>,
    pub present: Option<Present>,
    pub unchanged: Option<bool>,
    pub nodes: u32,
    pub counters: Vec<(NameId, i64)>,
}

impl Frame {
    /// Its end, or its begin when it never finished.
    pub fn end_or_begin(&self) -> u64 {
        self.end.unwrap_or(self.begin)
    }

    /// The core's part: begin to end, µs.
    pub fn core_us(&self) -> u64 {
        self.end_or_begin() - self.begin
    }

    /// Its work: the core's part and the driver's render, not vsync's
    /// wait, µs.
    pub fn work_us(&self) -> u64 {
        self.core_us() + self.present.map_or(0, |p| p.render)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Input {
    pub t: u64,
    pub kind: NameId,
    pub os_t: Option<u64>,
    pub count: u32,
    pub until: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Event {
    pub t: u64,
    pub kind: NameId,
    pub node: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Request {
    pub t: u64,
    pub why: NameId,
    pub at: NameId,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gauge {
    pub t: u64,
    pub name: NameId,
    pub value: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mark {
    pub t: u64,
    pub track: Track,
    pub name: NameId,
}

/// Every record of a recording, placed: frames in order, everything else
/// sorted by time.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Timeline {
    pub frames: Vec<Frame>,
    /// Sorted by start; a parent before its children at one start.
    pub spans: Vec<Span>,
    pub inputs: Vec<Input>,
    pub events: Vec<Event>,
    pub wakes: Vec<(u64, NameId)>,
    pub requests: Vec<Request>,
    pub loops: Vec<(u64, LoopEvent)>,
    pub gauges: Vec<Gauge>,
    pub marks: Vec<Mark>,
    pub notes: Vec<(u64, String)>,
    /// The first and the last time any record is placed at.
    pub start: u64,
    pub end: u64,
}

impl Timeline {
    pub fn of(records: &[Record]) -> Timeline {
        let mut tl = Timeline::default();
        let mut open: HashMap<u64, usize> = HashMap::new();
        let mut first = u64::MAX;
        let mut last = 0u64;
        for r in records {
            if let Some(t) = r.t() {
                first = first.min(t);
                let e = match r {
                    Record::Span(s) => s.end(),
                    Record::Input { until, .. } => *until,
                    _ => t,
                };
                last = last.max(e);
            }
            match r {
                Record::FrameBegin { t, frame, causes } => {
                    let gap_from = tl.frames.last().map_or(0, Frame::end_or_begin);
                    open.insert(*frame, tl.frames.len());
                    tl.frames.push(Frame {
                        frame: *frame,
                        begin: *t,
                        end: None,
                        gap_from,
                        causes: causes.clone(),
                        holders: Vec::new(),
                        present: None,
                        unchanged: None,
                        nodes: 0,
                        counters: Vec::new(),
                    });
                }
                Record::Holder {
                    frame,
                    what,
                    name,
                    node,
                } => {
                    if let Some(&i) = open.get(frame) {
                        tl.frames[i].holders.push(Holder {
                            what: *what,
                            name: *name,
                            node: *node,
                        });
                    }
                }
                Record::FrameEnd {
                    t,
                    frame,
                    unchanged,
                    nodes,
                } => {
                    if let Some(&i) = open.get(frame) {
                        let f = &mut tl.frames[i];
                        f.end = Some(*t);
                        f.unchanged = *unchanged;
                        f.nodes = *nodes;
                    }
                }
                Record::Present {
                    t,
                    frame,
                    render,
                    wait,
                } => {
                    if let Some(&i) = open.get(frame) {
                        tl.frames[i].present = Some(Present {
                            t: *t,
                            render: *render,
                            wait: *wait,
                        });
                    }
                }
                Record::Counter {
                    frame, name, value, ..
                } => {
                    if let Some(&i) = open.get(frame) {
                        tl.frames[i].counters.push((*name, *value));
                    }
                }
                Record::Span(s) => tl.spans.push(s.clone()),
                Record::Input {
                    t,
                    kind,
                    os_t,
                    count,
                    until,
                } => tl.inputs.push(Input {
                    t: *t,
                    kind: *kind,
                    os_t: *os_t,
                    count: *count,
                    until: *until,
                }),
                Record::Event { t, kind, node } => tl.events.push(Event {
                    t: *t,
                    kind: *kind,
                    node: *node,
                }),
                Record::Wake { t, name } => tl.wakes.push((*t, *name)),
                Record::Request { t, why, at } => tl.requests.push(Request {
                    t: *t,
                    why: *why,
                    at: *at,
                }),
                Record::Loop { t, event } => tl.loops.push((*t, *event)),
                Record::Gauge { t, name, value } => tl.gauges.push(Gauge {
                    t: *t,
                    name: *name,
                    value: *value,
                }),
                Record::Mark { t, track, name } => tl.marks.push(Mark {
                    t: *t,
                    track: *track,
                    name: *name,
                }),
                Record::Note { t, text } => tl.notes.push((*t, text.clone())),
                Record::Session(_) | Record::Name { .. } | Record::Track { .. } => {}
            }
        }
        tl.spans
            .sort_by(|a, b| a.start.cmp(&b.start).then(b.dur.cmp(&a.dur)));
        tl.inputs.sort_by_key(|i| i.t);
        tl.events.sort_by_key(|e| e.t);
        tl.wakes.sort_by_key(|w| w.0);
        tl.requests.sort_by_key(|r| r.t);
        tl.loops.sort_by_key(|l| l.0);
        tl.gauges.sort_by_key(|g| g.t);
        tl.marks.sort_by_key(|m| m.t);
        if first != u64::MAX {
            tl.start = first;
            tl.end = last;
        }
        tl
    }

    /// The spans that start in `[from, to)`.
    pub fn spans_in(&self, from: u64, to: u64) -> &[Span] {
        let a = self.spans.partition_point(|s| s.start < from);
        let b = self.spans.partition_point(|s| s.start < to);
        &self.spans[a..b]
    }

    /// The inputs that arrived in `[from, to)`.
    pub fn inputs_in(&self, from: u64, to: u64) -> &[Input] {
        let a = self.inputs.partition_point(|i| i.t < from);
        let b = self.inputs.partition_point(|i| i.t < to);
        &self.inputs[a..b]
    }

    /// The events handed out in `[from, to)`.
    pub fn events_in(&self, from: u64, to: u64) -> &[Event] {
        let a = self.events.partition_point(|e| e.t < from);
        let b = self.events.partition_point(|e| e.t < to);
        &self.events[a..b]
    }

    /// The loop's records in `[from, to)`.
    pub fn loops_in(&self, from: u64, to: u64) -> &[(u64, LoopEvent)] {
        let a = self.loops.partition_point(|l| l.0 < from);
        let b = self.loops.partition_point(|l| l.0 < to);
        &self.loops[a..b]
    }

    /// The wakes in `[from, to)`.
    pub fn wakes_in(&self, from: u64, to: u64) -> &[(u64, NameId)] {
        let a = self.wakes.partition_point(|w| w.0 < from);
        let b = self.wakes.partition_point(|w| w.0 < to);
        &self.wakes[a..b]
    }

    /// The marks in `[from, to)`.
    pub fn marks_in(&self, from: u64, to: u64) -> &[Mark] {
        let a = self.marks.partition_point(|m| m.t < from);
        let b = self.marks.partition_point(|m| m.t < to);
        &self.marks[a..b]
    }

    /// The requests in `[from, to)`.
    pub fn requests_in(&self, from: u64, to: u64) -> &[Request] {
        let a = self.requests.partition_point(|r| r.t < from);
        let b = self.requests.partition_point(|r| r.t < to);
        &self.requests[a..b]
    }

    /// Whether a driver recorded its loop at all: what `blocked` needs.
    pub fn has_loop(&self) -> bool {
        !self.loops.is_empty()
    }

    /// A span's own time: its length less its children's.
    pub fn self_us(&self, s: &Span) -> u64 {
        // A child counted up to its parent's end: one that runs past it
        // (cut there, or timed loosely) takes no more than the parent had.
        let kids: u64 = self
            .spans_in(s.start, s.end().max(s.start + 1))
            .iter()
            .filter(|k| k.parent == s.id && k.id != s.id)
            .map(|k| k.end().min(s.end()).saturating_sub(k.start))
            .sum();
        s.dur.saturating_sub(kids)
    }
}
