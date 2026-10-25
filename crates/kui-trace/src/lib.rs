//! kui's recording format (`docs/adr/0058-a-recording-is-a-timeline.md`):
//! a timeline of what happened in a kui app, in frames and between them.
//!
//! One clock — µs since the recording began — and [`Record`]s placed on
//! it: frames begun and finished with why they ran and who held them,
//! presents, nested spans with their CPU time and the node they began
//! under, inputs arriving, events handed to the app, wakes, frame
//! requests, the driver's loop going to sleep and waking, counters,
//! gauges, marks and notes. Frames are intervals on the clock; what
//! happens after one ends and before the next begins is the next one's
//! gap.
//!
//! `kui-core` writes it (the devtools' Perf tab records it, and
//! `KUI_PERF_TRACE=path` streams it to a file); this crate reads it with
//! no kui at all:
//!
//! ```
//! use kui_trace::{Record, Track, Trace, Span, Origin};
//!
//! let mut t = Trace::new();
//! let (build, _) = t.names.get("build");
//! t.records.push(Record::FrameBegin { t: 0, frame: 1, causes: vec![] });
//! t.records.push(Record::Span(Span {
//!     track: Track::MAIN, id: 1, parent: 0, name: build, origin: Origin::Kui,
//!     start: 10, dur: 400, cpu: None, node: 0, cut: false,
//! }));
//! t.records.push(Record::FrameEnd { t: 500, frame: 1, unchanged: Some(false), nodes: 12 });
//! let back = Trace::from_bytes(&t.to_bytes()).unwrap();
//! let tl = back.timeline();
//! assert_eq!(tl.frames[0].core_us(), 500);
//! assert_eq!(back.name(tl.spans[0].name), "build");
//! ```
//!
//! [`analysis`] reads a timeline the way the Perf tab does — phases, an
//! input's path to the screen, a frame's verdict, burns, spans grouped by
//! name and origin — and [`adapters`] write it out for Chrome's trace
//! viewer, Perfetto, speedscope and the Firefox Profiler.

pub mod adapters;
pub mod analysis;
mod codec;
pub mod diff;
pub mod dump;
mod model;
mod record;

pub use codec::{Encoder, Error, MAGIC, Reader, VERSION, Writer, header};
pub use model::{
    Event, Frame, Gauge, Holder, Input, Mark, Names, Present, Request, Timeline, Trace,
};
pub use record::{LoopEvent, NameId, Origin, Record, Session, Span, Track};

#[cfg(test)]
mod tests;
