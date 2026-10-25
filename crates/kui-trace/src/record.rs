//! The records a recording is made of (ADR 0058, decision 2): each placed
//! on one clock — µs since the recording began — and, where it says, on a
//! track. Frames are intervals on that clock, not its unit: what happens
//! after one frame ends and before the next begins is that next frame's
//! gap.
//!
//! No record holds a payload, typed text or a concealed field's
//! anything: kinds, keys, labels and times. A recording goes into bug
//! reports.

/// A string written once per recording by a [`Record::Name`]; `0` is the
/// empty string, never written.
pub type NameId = u32;

/// A row of the timeline: [`Track::MAIN`] the thread the core runs on,
/// [`Track::LOOP`] the driver's loop, and one per name `perf_measure` was
/// given, declared by a [`Record::Track`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Track(pub u32);

impl Track {
    pub const MAIN: Track = Track(0);
    pub const LOOP: Track = Track(1);
}

/// Who a span or a measurement is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Origin {
    /// kui's own: `build`, `layout`, `emit`, the devtools, a fill.
    Kui,
    /// The app.
    Host,
    /// An extension, by its namespace.
    Extension(NameId),
}

/// What the recording was made of, once, first.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Session {
    /// The kui version that wrote it.
    pub kui: String,
    /// The app's name, as its window titled it.
    pub app: String,
    /// `macos`, `windows`, `linux`, …
    pub platform: String,
    /// The display's refresh rate, and the frame budget it makes.
    pub refresh_hz: f32,
    pub budget_ms: f32,
    pub scale: f32,
    /// The wall clock at 0, ms since the Unix epoch.
    pub wall_ms: u64,
}

/// A span closed: a name, a start and a length on its track, nested in
/// its parent.
#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub track: Track,
    /// Unique in the recording; `0` is no span.
    pub id: u32,
    /// The span it was opened inside, or `0`.
    pub parent: u32,
    pub name: NameId,
    pub origin: Origin,
    pub start: u64,
    pub dur: u64,
    /// The thread's time on a CPU over it, where the platform reads one.
    pub cpu: Option<u64>,
    /// The key of the node the builder had open as it began, or `0`.
    pub node: u64,
    /// Left open when its frame ended, and closed there.
    pub cut: bool,
}

impl Span {
    pub fn end(&self) -> u64 {
        self.start + self.dur
    }
}

/// What the driver's loop did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LoopEvent {
    /// Went to sleep, until a time when there was one.
    Sleep { until: Option<u64> },
    /// Woke, and why (`input`, `wake`, `timer`, `redraw`, …).
    Woke { by: NameId },
    /// Handled an OS event that never became input.
    Os { kind: NameId },
}

/// One record.
#[derive(Clone, Debug, PartialEq)]
pub enum Record {
    Session(Session),
    Name {
        id: NameId,
        text: String,
    },
    Track {
        id: Track,
        name: NameId,
    },
    /// A frame begun, with why it runs: the cause kinds and the named
    /// ones (`wake: parser`, `request: caret blink`).
    FrameBegin {
        t: u64,
        frame: u64,
        causes: Vec<NameId>,
    },
    /// Who held the frame the last one left owed: what for and its name.
    Holder {
        frame: u64,
        what: NameId,
        name: NameId,
        node: u64,
    },
    /// A frame finished: whether it drew what the one before drew, and
    /// how many nodes it held.
    FrameEnd {
        t: u64,
        frame: u64,
        unchanged: Option<bool>,
        nodes: u32,
    },
    /// A frame drawn: the driver's render and vsync's wait, µs.
    Present {
        t: u64,
        frame: u64,
        render: u64,
        wait: u64,
    },
    Span(Span),
    /// Input arriving: its kind and, for a run of one kind (pointer
    /// moves), how many and until when — never before `t`, which is what
    /// a single input's `until` is.
    Input {
        t: u64,
        kind: NameId,
        os_t: Option<u64>,
        count: u32,
        until: u64,
    },
    /// An event handed to the app: its kind and its node.
    Event {
        t: u64,
        kind: NameId,
        node: u64,
    },
    Wake {
        t: u64,
        name: NameId,
    },
    /// A frame asked for: why, and the source line that asked.
    Request {
        t: u64,
        why: NameId,
        at: NameId,
    },
    Loop {
        t: u64,
        event: LoopEvent,
    },
    /// A frame's counter: its total over the frame.
    Counter {
        t: u64,
        frame: u64,
        name: NameId,
        value: i64,
    },
    /// A reading set.
    Gauge {
        t: u64,
        name: NameId,
        value: f64,
    },
    /// An instant.
    Mark {
        t: u64,
        track: Track,
        name: NameId,
    },
    Note {
        t: u64,
        text: String,
    },
}

impl Record {
    /// The time it is placed at, for one that has one.
    pub fn t(&self) -> Option<u64> {
        Some(match self {
            Record::Session(_)
            | Record::Name { .. }
            | Record::Track { .. }
            | Record::Holder { .. } => return None,
            Record::FrameBegin { t, .. }
            | Record::FrameEnd { t, .. }
            | Record::Present { t, .. }
            | Record::Input { t, .. }
            | Record::Event { t, .. }
            | Record::Wake { t, .. }
            | Record::Request { t, .. }
            | Record::Loop { t, .. }
            | Record::Counter { t, .. }
            | Record::Gauge { t, .. }
            | Record::Mark { t, .. }
            | Record::Note { t, .. } => *t,
            Record::Span(s) => s.start,
        })
    }
}
