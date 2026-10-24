//! The recording (`docs/adr/0058-a-recording-is-a-timeline.md`): what a
//! window's core did, in frames and between them, as `kui-trace` records
//! on one clock.
//!
//! Nothing is recorded unless something listens (decision 1): the
//! devtools' Perf tab recording or on show, `KUI_PERF_TRACE` naming a
//! file, a `tracing` subscriber under the feature. Off, every door here is
//! one test of [`Recorder::why`] — no clock read, no allocation — and
//! [`Core::perf_measuring`] says so to an app that would read clocks of its
//! own. On, the frame trace (F111) is held on too, for the frame's causes
//! by name, its holders and whether it drew anything new, and put back as
//! the app had it after.
//!
//! The core records on its own (decision 4): a frame's begin and end, the
//! spans `build` (from `begin_frame`'s end to `Ui::finish`), `fill
//! <namespace>` for each extension's fill, `devtools`, `layout` and
//! `emit`, the texts shaped and glyphs rasterized as counters, every input
//! as it arrives (a run of pointer moves as one record), every event as it
//! leaves, and every frame asked for. The drivers add what only they see
//! (decision 5) through [`Core::note_loop`], [`Core::push_frame_sample`]
//! and a `handler` span round each call into the app; an app adds its own
//! spans, counters, gauges, marks and measures (decisions 6, 7).

use std::collections::VecDeque;
use std::panic::Location;
use std::time::Instant;

use kui_trace::{LoopEvent, NameId, Names, Origin as TOrigin, Record, Session, Track};
use rustc_hash::FxHashMap;

use super::*;
use crate::input::InputEvent;

/// How many frames the recording keeps in memory while the Perf tab
/// records or a file is written: a minute at 30 frames a second.
pub const RING_FRAMES: usize = 1800;
/// How many while the tab only shows the recent frames.
pub const LIVE_FRAMES: usize = 120;
/// How many spans a frame keeps; the rest are counted.
const MAX_SPANS: usize = 1024;

/// Why measuring is on: a bit each.
pub(crate) mod why {
    /// The Perf tab's record button, or `set_perf_recording`.
    pub const RECORDING: u8 = 1;
    /// The Perf tab on show, docked in this window.
    pub const LIVE: u8 = 2;
    /// `KUI_PERF_TRACE`, or `perf_trace_to`.
    pub const FILE: u8 = 4;
    /// A `tracing` subscriber listening for target `kui`.
    #[cfg_attr(not(feature = "tracing"), allow(dead_code))]
    pub const TRACING: u8 = 8;
}

/// A span begun, for [`Core::span_end`]. [`SpanId::NONE`] while nothing
/// is recorded, which `span_end` ignores.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SpanId(pub u32);

impl SpanId {
    pub const NONE: SpanId = SpanId(0);
}

/// What a driver's loop did, for the recording ([`Core::note_loop`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LoopNote<'a> {
    /// Going to sleep, for at most this many ms when there is a deadline.
    Sleep { for_ms: Option<f64> },
    /// Woken, and by what: `input`, `wake`, `timer`, `redraw`, …
    Woke(&'a str),
    /// An OS event handled that never became input.
    Os(&'a str),
}

struct Open {
    id: u32,
    parent: u32,
    name: NameId,
    origin: TOrigin,
    start: u64,
    cpu0: Option<u64>,
    node: u64,
    #[cfg(feature = "tracing")]
    traced: Option<tracing::Span>,
}

enum Sink {
    Stream(kui_trace::Writer<std::io::BufWriter<std::fs::File>>),
    Convert {
        path: String,
        format: kui_trace::adapters::Format,
        trace: Box<kui_trace::Trace>,
    },
}

/// The recording's state on a core.
#[derive(Default)]
pub(crate) struct Recorder {
    /// Why measuring is on ([`why`]); `0` is off.
    pub(crate) why: u8,
    epoch: Option<Instant>,
    names: Names,
    session: Option<Session>,
    tracks: Vec<(Track, NameId)>,
    ring: VecDeque<Record>,
    ring_frames: usize,
    stack: Vec<Open>,
    next_id: u32,
    /// The frame being built, from `begin_frame` to `finish_frame`.
    frame: Option<u64>,
    /// The last frame finished: what a present is for.
    last_frame: u64,
    frame_t: u64,
    spans_in_frame: usize,
    /// Spans a frame had past [`MAX_SPANS`].
    pub(crate) dropped: u64,
    /// A run of pointer moves: first, last, how many, its kind.
    run: Option<(u64, u64, u32, NameId)>,
    counters: Vec<(NameId, i64)>,
    /// Causes with names for the next frame: `wake: parser`.
    named: Vec<NameId>,
    shaped_at: u64,
    rasterized_at: u64,
    build: SpanId,
    /// The frame trace as the app had it, while measuring holds it on.
    trace_was: Option<bool>,
    ext_names: FxHashMap<OriginId, NameId>,
    sink: Option<Sink>,
    /// `request_frame_for`'s names, interned as `&'static str` so a
    /// [`crate::FrameRequest`] stays `Copy` (ADR 0058, decision 8).
    interned: Vec<&'static str>,
    warned_names: bool,
}

/// At most this many distinct `request_frame_for` names a session.
const MAX_REQUEST_NAMES: usize = 256;

#[cfg(unix)]
fn cpu_us() -> Option<u64> {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `clock_gettime` fills the timespec it is handed.
    let r = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) };
    (r == 0).then(|| ts.tv_sec as u64 * 1_000_000 + ts.tv_nsec as u64 / 1_000)
}

/// Windows' thread times tick every 15.6 ms: no reading worth a span's.
#[cfg(not(unix))]
fn cpu_us() -> Option<u64> {
    None
}

impl Recorder {
    fn now(&mut self) -> u64 {
        self.epoch
            .get_or_insert_with(Instant::now)
            .elapsed()
            .as_micros() as u64
    }

    fn name(&mut self, s: &str) -> NameId {
        let (id, new) = self.names.get(s);
        if new && let Some(Sink::Stream(w)) = &mut self.sink {
            let _ = w.write(&Record::Name {
                id,
                text: s.to_string(),
            });
        }
        id
    }

    fn cap(&self) -> usize {
        if self.why & (why::RECORDING | why::FILE) != 0 {
            RING_FRAMES
        } else {
            LIVE_FRAMES
        }
    }

    fn flush_run(&mut self) {
        if let Some((t, until, count, kind)) = self.run.take() {
            self.keep(Record::Input {
                t,
                kind,
                os_t: None,
                count,
                until,
            });
        }
    }

    fn push(&mut self, r: Record) {
        self.flush_run();
        self.keep(r);
    }

    fn keep(&mut self, r: Record) {
        match &mut self.sink {
            Some(Sink::Stream(w)) => {
                let _ = w.write(&r);
            }
            Some(Sink::Convert { trace, .. }) => trace.records.push(r.clone()),
            None => {}
        }
        if matches!(r, Record::FrameBegin { .. }) {
            self.ring_frames += 1;
            let cap = self.cap();
            while self.ring_frames > cap {
                // The oldest frame and everything before the next one.
                let mut seen = false;
                while let Some(front) = self.ring.front() {
                    let is_frame = matches!(front, Record::FrameBegin { .. });
                    if is_frame && seen {
                        break;
                    }
                    seen |= is_frame;
                    self.ring.pop_front();
                }
                self.ring_frames -= 1;
            }
        }
        self.ring.push_back(r);
    }

    fn clear(&mut self) {
        self.ring.clear();
        self.ring_frames = 0;
        self.dropped = 0;
    }

    fn track(&mut self, name: &str) -> Track {
        let n = self.name(name);
        if let Some((t, _)) = self.tracks.iter().find(|(_, tn)| *tn == n) {
            return *t;
        }
        let t = Track(2 + self.tracks.len() as u32);
        self.tracks.push((t, n));
        if let Some(Sink::Stream(w)) = &mut self.sink {
            let _ = w.write(&Record::Track { id: t, name: n });
        }
        t
    }

    /// Everything kept, as a recording.
    fn trace(&self) -> kui_trace::Trace {
        kui_trace::Trace {
            session: self.session.clone(),
            names: self.names.clone(),
            tracks: self.tracks.clone(),
            records: self.ring.iter().cloned().collect(),
        }
    }
}

impl Core {
    /// Whether anything is being recorded: the devtools' Perf tab
    /// recording or on show, `KUI_PERF_TRACE`, a `tracing` subscriber.
    /// Off, every perf door is a no-op; an app reads this to skip reading
    /// clocks of its own for them.
    pub fn perf_measuring(&self) -> bool {
        self.perf.why != 0
    }

    /// Starts or stops a recording, as the Perf tab's record button does:
    /// starting drops what was kept, stopping keeps it for
    /// [`Self::perf_trace`].
    pub fn set_perf_recording(&mut self, on: bool) {
        if on && self.perf.why & why::RECORDING == 0 {
            self.perf.clear();
        }
        self.perf_set(why::RECORDING, on);
    }

    /// Whether a recording is running.
    pub fn perf_recording(&self) -> bool {
        self.perf.why & why::RECORDING != 0
    }

    pub(crate) fn perf_set(&mut self, bit: u8, on: bool) {
        let was = self.perf.why;
        if on {
            self.perf.why |= bit;
        } else {
            self.perf.why &= !bit;
        }
        if was == 0 && self.perf.why != 0 {
            // Measuring begins: a session, and the frame trace held on.
            self.perf.trace_was = Some(self.frame_trace());
            self.set_frame_trace(true);
            if self.perf.session.is_none() {
                self.perf.epoch = None;
                let wall_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_millis() as u64);
                self.perf.session = Some(Session {
                    kui: env!("CARGO_PKG_VERSION").to_string(),
                    app: self.window_title_for_perf(),
                    platform: std::env::consts::OS.to_string(),
                    refresh_hz: self.env.refresh_hz.unwrap_or(0.0),
                    budget_ms: self.env.frame_budget_ms(),
                    scale: self.scale,
                    wall_ms,
                });
                let s = self.perf.session.clone().unwrap();
                if let Some(Sink::Stream(w)) = &mut self.perf.sink {
                    let _ = w.write(&Record::Session(s));
                }
            }
        } else if was != 0 && self.perf.why == 0 {
            // Measuring ends: the trace as the app had it; what is open
            // closes.
            self.perf.stack.clear();
            self.perf.run = None;
            if let Some(was) = self.perf.trace_was.take() {
                self.set_frame_trace(was);
            }
        }
    }

    fn window_title_for_perf(&self) -> String {
        self.window_title.clone().unwrap_or_default()
    }

    fn perf_origin(&mut self) -> TOrigin {
        match self.origin {
            OriginId::HOST => TOrigin::Host,
            OriginId::DEVTOOLS => TOrigin::Kui,
            o => {
                let n = match self.perf.ext_names.get(&o) {
                    Some(n) => *n,
                    None => self.perf.name(&format!("origin {}", o.0)),
                };
                TOrigin::Extension(n)
            }
        }
    }

    /// Begins a span named `name`: under the app's origin (or the
    /// extension's whose fill runs), on the main track, inside the span
    /// open now, recording the node the builder has open. End it with
    /// [`Self::span_end`]; a span left open when its frame ends is closed
    /// there and marked cut. [`SpanId::NONE`] while nothing is recorded.
    pub fn span_begin(&mut self, name: &str) -> SpanId {
        if self.perf.why == 0 {
            return SpanId::NONE;
        }
        let origin = self.perf_origin();
        self.span_begin_as(name, origin)
    }

    pub(crate) fn span_begin_as(&mut self, name: &str, origin: TOrigin) -> SpanId {
        if self.perf.why == 0 {
            return SpanId::NONE;
        }
        let name_id = self.perf.name(name);
        let node = self
            .stack
            .last()
            .and_then(|&i| self.tree.keys.get(i as usize))
            .map_or(0, |k| k.0);
        self.perf.next_id += 1;
        let id = self.perf.next_id;
        let parent = self.perf.stack.last().map_or(0, |o| o.id);
        let start = self.perf.now();
        #[cfg(feature = "tracing")]
        let traced = (self.perf.why & why::TRACING != 0).then(|| {
            let s = tracing::trace_span!(target: "kui", "kui", name = name, node = node);
            if let Some(id) = s.id() {
                tracing::dispatcher::get_default(|d| d.enter(&id));
            }
            s
        });
        self.perf.stack.push(Open {
            id,
            parent,
            name: name_id,
            origin,
            start,
            cpu0: cpu_us(),
            node,
            #[cfg(feature = "tracing")]
            traced,
        });
        SpanId(id)
    }

    /// Ends the span `id` began, and any opened inside it and left open.
    pub fn span_end(&mut self, id: SpanId) {
        if id == SpanId::NONE || self.perf.why == 0 {
            return;
        }
        let Some(pos) = self.perf.stack.iter().rposition(|o| o.id == id.0) else {
            return;
        };
        let end = self.perf.now();
        let cpu = cpu_us();
        while self.perf.stack.len() > pos {
            let o = self.perf.stack.pop().unwrap();
            self.perf_close(o, end, cpu, false);
        }
    }

    fn perf_close(&mut self, o: Open, end: u64, cpu: Option<u64>, cut: bool) {
        #[cfg(feature = "tracing")]
        if let Some(s) = &o.traced
            && let Some(id) = s.id()
        {
            tracing::dispatcher::get_default(|d| d.exit(&id));
        }
        if self.perf.frame.is_some() {
            self.perf.spans_in_frame += 1;
            if self.perf.spans_in_frame > MAX_SPANS {
                self.perf.dropped += 1;
                return;
            }
        }
        self.perf.push(Record::Span(kui_trace::Span {
            track: Track::MAIN,
            id: o.id,
            parent: o.parent,
            name: o.name,
            origin: o.origin,
            start: o.start,
            dur: end.saturating_sub(o.start),
            cpu: o.cpu0.zip(cpu).map(|(a, b)| b.saturating_sub(a)),
            node: o.node,
            cut,
        }));
    }

    /// A span whose length was timed elsewhere — a parse on another
    /// thread, a job — ending now, on the track named `name`.
    pub fn perf_measure(&mut self, name: &str, ms: f64) {
        if self.perf.why == 0 {
            return;
        }
        let origin = self.perf_origin();
        let track = self.perf.track(name);
        let name_id = self.perf.name(name);
        let end = self.perf.now();
        let dur = (ms.max(0.0) * 1000.0) as u64;
        self.perf.next_id += 1;
        let id = self.perf.next_id;
        self.perf.push(Record::Span(kui_trace::Span {
            track,
            id,
            parent: 0,
            name: name_id,
            origin,
            start: end.saturating_sub(dur),
            dur,
            cpu: None,
            node: 0,
            cut: false,
        }));
    }

    /// Adds `n` to this frame's counter `name`.
    pub fn perf_count(&mut self, name: &str, n: i64) {
        if self.perf.why == 0 {
            return;
        }
        let id = self.perf.name(name);
        match self.perf.counters.iter_mut().find(|(c, _)| *c == id) {
            Some((_, v)) => *v += n,
            None => self.perf.counters.push((id, n)),
        }
    }

    /// Sets the reading `name` — a footprint, a buffer's bytes — held
    /// until set again.
    pub fn perf_gauge(&mut self, name: &str, value: f64) {
        if self.perf.why == 0 {
            return;
        }
        let t = self.perf.now();
        let name = self.perf.name(name);
        self.perf.push(Record::Gauge { t, name, value });
    }

    /// Puts an instant on the timeline: a file opened, a command run.
    pub fn perf_mark(&mut self, name: &str) {
        if self.perf.why == 0 {
            return;
        }
        let t = self.perf.now();
        let name = self.perf.name(name);
        self.perf.push(Record::Mark {
            t,
            track: Track::MAIN,
            name,
        });
    }

    /// A wake named `name` reached this window: the next frame's causes
    /// read `wake: name`. For a driver or a host whose thread woke the
    /// loop; Rust's `Waker::named` calls it for the app.
    pub fn note_wake(&mut self, name: &str) {
        self.note_frame_cause(crate::runtime::cause::FrameCause::WAKE);
        if self.perf.why == 0 {
            return;
        }
        let t = self.perf.now();
        let n = self.perf.name(name);
        let cause = self.perf.name(&format!("wake: {name}"));
        if !self.perf.named.contains(&cause) {
            self.perf.named.push(cause);
        }
        self.perf.push(Record::Wake { t, name: n });
    }

    /// Asks for a frame as [`Self::request_frame`] does, and names it:
    /// `owed_by` lists it under `why` in place of `"request_frame"`, and
    /// a recording reads `request: why`.
    #[track_caller]
    pub fn request_frame_for(&mut self, why: &str) {
        self.taint_kept("it asked for a frame");
        self.frame_requested = true;
        let name = self.perf_intern(why);
        self.trace_request_named(name);
        if self.perf.why != 0 {
            let cause = self.perf.name(&format!("request: {why}"));
            if !self.perf.named.contains(&cause) {
                self.perf.named.push(cause);
            }
        }
    }

    fn perf_intern(&mut self, why: &str) -> &'static str {
        if let Some(s) = self.perf.interned.iter().find(|s| **s == why) {
            return s;
        }
        if self.perf.interned.len() >= MAX_REQUEST_NAMES {
            if !std::mem::replace(&mut self.perf.warned_names, true) {
                self.diag
                    .raise(crate::diag::perf_names_exhausted(MAX_REQUEST_NAMES));
            }
            return "request_frame";
        }
        let s: &'static str = Box::leak(why.to_string().into_boxed_str());
        self.perf.interned.push(s);
        s
    }

    /// What the driver's loop did, for the recording.
    pub fn note_loop(&mut self, note: LoopNote<'_>) {
        if self.perf.why == 0 {
            return;
        }
        let t = self.perf.now();
        let event = match note {
            LoopNote::Sleep { for_ms } => LoopEvent::Sleep {
                until: for_ms.map(|ms| t + (ms.max(0.0) * 1000.0) as u64),
            },
            LoopNote::Woke(by) => LoopEvent::Woke {
                by: self.perf.name(by),
            },
            LoopNote::Os(kind) => LoopEvent::Os {
                kind: self.perf.name(kind),
            },
        };
        self.perf.push(Record::Loop { t, event });
    }

    /// The driver's timing of the frame just drawn, into
    /// [`Self::stats`](crate::stats) for the latency graph, and into the
    /// recording as the frame's present.
    pub fn push_frame_sample(&mut self, sample: crate::stats::FrameSample) {
        self.stats.push(sample);
        if self.perf.why == 0 || self.perf.last_frame == 0 {
            return;
        }
        let t = self.perf.now();
        self.perf.push(Record::Present {
            t,
            frame: self.perf.last_frame,
            render: (sample.render_ms.max(0.0) * 1000.0) as u64,
            wait: (sample.wait_ms.max(0.0) * 1000.0) as u64,
        });
    }

    /// What is kept — the last recording, or the frames the Perf tab
    /// shows — as `kui-trace` bytes.
    pub fn perf_trace(&self) -> Vec<u8> {
        self.perf.trace().to_bytes()
    }

    /// What is kept, as a recording to read.
    pub fn perf_trace_read(&self) -> kui_trace::Trace {
        self.perf.trace()
    }

    /// Records from now on and writes to `path` as it goes: `.kuitrace`
    /// streamed, a format an adapter writes (`.json` Chrome's, `.pftrace`
    /// Perfetto's, `.speedscope.json`, `.firefox.json`) kept and written
    /// by [`Self::perf_flush`] or when the core is dropped.
    pub fn perf_trace_to(&mut self, path: &str) -> std::io::Result<()> {
        use kui_trace::adapters::Format;
        let format = Format::from_path(path);
        self.perf.sink = Some(match format {
            Format::Kuitrace => {
                let f = std::fs::File::create(path)?;
                let mut w = kui_trace::Writer::new(std::io::BufWriter::new(f))?;
                if let Some(s) = &self.perf.session {
                    w.write(&Record::Session(s.clone()))?;
                }
                for (id, text) in self.perf.names.all().iter().enumerate().skip(1) {
                    w.write(&Record::Name {
                        id: id as NameId,
                        text: text.clone(),
                    })?;
                }
                Sink::Stream(w)
            }
            format => {
                std::fs::write(path, b"")?;
                Sink::Convert {
                    path: path.to_string(),
                    format,
                    trace: Box::default(),
                }
            }
        });
        self.perf_set(why::FILE, true);
        Ok(())
    }

    /// `KUI_PERF_TRACE=path`: records to `path` from the first frame,
    /// what the runners read at launch. Whether it named one that opened.
    pub fn perf_from_env(&mut self) -> bool {
        let Some(path) = std::env::var_os("KUI_PERF_TRACE") else {
            return false;
        };
        let path = path.to_string_lossy().into_owned();
        match self.perf_trace_to(&path) {
            Ok(()) => true,
            Err(e) => {
                self.diag
                    .raise(crate::diag::perf_trace_failed(&path, &e.to_string()));
                false
            }
        }
    }

    /// Writes out what a file being recorded to holds so far.
    pub fn perf_flush(&mut self) -> std::io::Result<()> {
        let names = self.perf.names.clone();
        let session = self.perf.session.clone();
        let tracks = self.perf.tracks.clone();
        match &mut self.perf.sink {
            Some(Sink::Stream(w)) => w.flush(),
            Some(Sink::Convert {
                path,
                format,
                trace,
            }) => {
                trace.names = names;
                trace.session = session;
                trace.tracks = tracks;
                std::fs::write(&*path, kui_trace::adapters::write(trace, *format))
            }
            None => Ok(()),
        }
    }

    /// The start of `begin_frame`: the frame's time.
    pub(crate) fn perf_frame_start(&mut self) {
        #[cfg(feature = "tracing")]
        {
            let listening = tracing::enabled!(target: "kui", tracing::Level::TRACE);
            if listening != (self.perf.why & why::TRACING != 0) {
                self.perf_set(why::TRACING, listening);
            }
        }
        if self.perf.why == 0 {
            return;
        }
        self.perf.frame_t = self.perf.now();
    }

    /// The end of `begin_frame`, once the frame has its number and the
    /// trace its causes and holders: the frame begun, and `build` open.
    pub(crate) fn perf_frame_begun(&mut self) {
        if self.perf.why == 0 {
            return;
        }
        let frame = self.frame_no;
        let mut causes: Vec<NameId> = self
            .frame_cause()
            .names()
            .map(|n| self.perf.name(n))
            .collect();
        causes.append(&mut self.perf.named);
        let t = self.perf.frame_t;
        self.perf.push(Record::FrameBegin { t, frame, causes });
        let by = self.owed_by().clone();
        let mut holders: Vec<(&'static str, String, u64)> = Vec::new();
        for (what, list) in [
            ("transition", &by.transitions),
            ("cycle", &by.cycles),
            ("exit", &by.departures),
            ("scroll", &by.scrolls),
            ("animate", &by.animate),
        ] {
            for h in list {
                let name = if h.slots.is_empty() {
                    h.name.clone()
                } else {
                    format!("{} ({})", h.name, h.slots.join(" "))
                };
                holders.push((what, name, h.key.0));
            }
        }
        if let Some(h) = &by.autoscroll {
            holders.push(("autoscroll", h.name.clone(), h.key.0));
        }
        for r in &by.requests {
            holders.push((
                "request",
                format!("{} · {}:{}", r.why, short_path(r.at.file()), r.at.line()),
                0,
            ));
        }
        for (what, name, node) in holders {
            let what = self.perf.name(what);
            let name = self.perf.name(&name);
            self.perf.push(Record::Holder {
                frame,
                what,
                name,
                node,
            });
        }
        self.perf.frame = Some(frame);
        self.perf.spans_in_frame = 0;
        self.perf.shaped_at = self.text.shaped;
        self.perf.rasterized_at = self.atlas.rasterized;
        self.perf.build = self.span_begin_as(kui_trace::analysis::BUILD, TOrigin::Kui);
    }

    /// The start of `Ui::finish`: the host's view is done.
    pub(crate) fn perf_build_done(&mut self) {
        let b = std::mem::take(&mut self.perf.build);
        self.span_end(b);
    }

    /// The end of `finish_frame`, once the trace knows whether the frame
    /// drew anything new: the frame finished, its counters, and whatever
    /// the app left open cut there.
    pub(crate) fn perf_frame_end(&mut self) {
        let Some(frame) = self.perf.frame.take() else {
            return;
        };
        if self.perf.why == 0 {
            return;
        }
        let end = self.perf.now();
        let cpu = cpu_us();
        while let Some(o) = self.perf.stack.pop() {
            self.perf_close(o, end, cpu, true);
        }
        let shaped = self.text.shaped - self.perf.shaped_at;
        let rasterized = self.atlas.rasterized - self.perf.rasterized_at;
        let mut counters = std::mem::take(&mut self.perf.counters);
        for (name, v) in [("texts shaped", shaped), ("glyphs rasterized", rasterized)] {
            if v > 0 {
                counters.push((self.perf.name(name), v as i64));
            }
        }
        for (name, value) in counters {
            self.perf.push(Record::Counter {
                t: end,
                frame,
                name,
                value,
            });
        }
        let unchanged = self.frame_unchanged();
        self.perf.push(Record::FrameEnd {
            t: end,
            frame,
            unchanged,
            nodes: self.tree.len() as u32,
        });
        self.perf.last_frame = frame;
    }

    /// An input as it arrives: a run of pointer moves as one record.
    pub(crate) fn perf_input(&mut self, ev: &InputEvent) {
        if self.perf.why == 0 {
            return;
        }
        let kind = crate::runtime::cause::FrameCause::of_input(ev);
        let Some(name) = kind.names().next() else {
            return;
        };
        let t = self.perf.now();
        let kind = self.perf.name(name);
        let pointer = matches!(ev, InputEvent::CursorMoved(_));
        if pointer {
            match &mut self.perf.run {
                Some((_, until, count, k)) if *k == kind => {
                    *until = t;
                    *count += 1;
                }
                _ => {
                    self.perf.flush_run();
                    self.perf.run = Some((t, t, 1, kind));
                }
            }
            return;
        }
        self.perf.push(Record::Input {
            t,
            kind,
            os_t: None,
            count: 1,
            until: t,
        });
    }

    /// Events on their way to the app: kind and node, nothing of their
    /// payload.
    pub(crate) fn perf_events(&mut self, out: &[UiEvent]) {
        if self.perf.why == 0 || out.is_empty() {
            return;
        }
        let t = self.perf.now();
        for ev in out {
            let kind = self.perf.name(ev.event.name());
            self.perf.push(Record::Event {
                t,
                kind,
                node: ev.key.0,
            });
        }
    }

    /// A frame asked for, by the app's door or the core's own.
    pub(crate) fn perf_request(&mut self, why: &'static str, at: &'static Location<'static>) {
        if self.perf.why == 0 {
            return;
        }
        let t = self.perf.now();
        let why = self.perf.name(why);
        let at = self
            .perf
            .name(&format!("{}:{}", short_path(at.file()), at.line()));
        self.perf.push(Record::Request { t, why, at });
    }

    /// An extension's fill begins: its namespace, for the spans under
    /// its origin, and a `fill <namespace>` span.
    pub(crate) fn perf_fill_begin(&mut self, origin: OriginId, namespace: &str) -> SpanId {
        if self.perf.why == 0 {
            return SpanId::NONE;
        }
        let ns = self.perf.name(namespace);
        self.perf.ext_names.insert(origin, ns);
        self.span_begin_as(
            &format!("{}{namespace}", kui_trace::analysis::FILL),
            TOrigin::Extension(ns),
        )
    }

    /// The Perf tab on show in this window, or not.
    pub(crate) fn perf_set_live(&mut self, on: bool) {
        if on != (self.perf.why & why::LIVE != 0) {
            self.perf_set(why::LIVE, on);
        }
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        match &mut self.sink {
            Some(Sink::Stream(w)) => {
                let _ = w.flush();
            }
            Some(Sink::Convert {
                path,
                format,
                trace,
            }) => {
                trace.names = self.names.clone();
                trace.session = self.session.clone();
                trace.tracks = self.tracks.clone();
                let _ = std::fs::write(&*path, kui_trace::adapters::write(trace, *format));
            }
            None => {}
        }
    }
}

/// A source path's last two parts, `src/app.rs`.
pub(crate) fn short_path(path: &str) -> &str {
    let mut cuts = path.rmatch_indices(['/', '\\']).map(|(i, _)| i);
    match (cuts.next(), cuts.next()) {
        (Some(_), Some(i)) => &path[i + 1..],
        _ => path,
    }
}

impl Ui<'_> {
    /// Runs `f` inside a span named `name` ([`Core::span_begin`]).
    pub fn span<R>(&mut self, name: &str, f: impl FnOnce(&mut Ui<'_>) -> R) -> R {
        let id = self.core().span_begin(name);
        let r = f(self);
        self.core().span_end(id);
        r
    }

    /// Asks for a frame and names why ([`Core::request_frame_for`]).
    #[track_caller]
    pub fn request_frame_for(&mut self, why: &str) {
        self.core().request_frame_for(why);
    }
}
