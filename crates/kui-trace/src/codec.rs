//! The bytes (ADR 0058, decision 3): `KUITRACE`, a version, then records,
//! each a tag and a length before its fields. Integers are LEB128
//! varints; a time is the zigzagged difference from the last time written,
//! so a span written as it closes, starting before the record before it,
//! costs a byte or two like any other. A reader skips a tag it does not
//! know by its length, and reads a file written as it records up to where
//! it stopped.

use std::io::{self, Write};

use crate::record::{LoopEvent, NameId, Origin, Record, Session, Span, Track};

/// The first eight bytes.
pub const MAGIC: &[u8; 8] = b"KUITRACE";
/// The format's version, after the magic, little-endian.
pub const VERSION: u16 = 1;

/// What went wrong reading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// Not a recording: the magic is missing.
    NotATrace,
    /// A version this reader does not read.
    Version(u16),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotATrace => write!(f, "not a kui recording (no KUITRACE header)"),
            Error::Version(v) => write!(f, "a version {v} recording; this reads {VERSION}"),
        }
    }
}

impl std::error::Error for Error {}

mod tag {
    pub const SESSION: u8 = 1;
    pub const NAME: u8 = 2;
    pub const TRACK: u8 = 3;
    pub const FRAME_BEGIN: u8 = 4;
    pub const HOLDER: u8 = 5;
    pub const FRAME_END: u8 = 6;
    pub const PRESENT: u8 = 7;
    pub const SPAN: u8 = 8;
    pub const INPUT: u8 = 9;
    pub const EVENT: u8 = 10;
    pub const WAKE: u8 = 11;
    pub const REQUEST: u8 = 12;
    pub const LOOP: u8 = 13;
    pub const COUNTER: u8 = 14;
    pub const GAUGE: u8 = 15;
    pub const MARK: u8 = 16;
    pub const NOTE: u8 = 17;
    pub const PLACEMENT: u8 = 18;
}

fn put_u(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let b = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            return;
        }
        out.push(b | 0x80);
    }
}

fn put_i(out: &mut Vec<u8>, v: i64) {
    put_u(out, ((v << 1) ^ (v >> 63)) as u64);
}

fn put_str(out: &mut Vec<u8>, s: &str) {
    put_u(out, s.len() as u64);
    out.extend_from_slice(s.as_bytes());
}

fn put_opt(out: &mut Vec<u8>, v: Option<u64>) {
    put_u(out, v.map_or(0, |v| v + 1));
}

/// Encodes records one after another, keeping the last time written.
#[derive(Default)]
pub struct Encoder {
    prev_t: u64,
    body: Vec<u8>,
}

impl Encoder {
    pub fn new() -> Self {
        Self::default()
    }

    fn t(&mut self, t: u64) {
        put_i(&mut self.body, t as i64 - self.prev_t as i64);
        self.prev_t = t;
    }

    /// `r` as bytes: its tag, its length and its fields, appended to `out`.
    pub fn encode(&mut self, r: &Record, out: &mut Vec<u8>) {
        self.body.clear();
        let tag = match r {
            Record::Session(s) => {
                put_str(&mut self.body, &s.kui);
                put_str(&mut self.body, &s.app);
                put_str(&mut self.body, &s.platform);
                self.body.extend_from_slice(&s.refresh_hz.to_le_bytes());
                self.body.extend_from_slice(&s.budget_ms.to_le_bytes());
                self.body.extend_from_slice(&s.scale.to_le_bytes());
                put_u(&mut self.body, s.wall_ms);
                tag::SESSION
            }
            Record::Name { id, text } => {
                put_u(&mut self.body, *id as u64);
                put_str(&mut self.body, text);
                tag::NAME
            }
            Record::Track { id, name } => {
                put_u(&mut self.body, id.0 as u64);
                put_u(&mut self.body, *name as u64);
                tag::TRACK
            }
            Record::FrameBegin { t, frame, causes } => {
                self.t(*t);
                put_u(&mut self.body, *frame);
                put_u(&mut self.body, causes.len() as u64);
                for c in causes {
                    put_u(&mut self.body, *c as u64);
                }
                tag::FRAME_BEGIN
            }
            Record::Holder {
                frame,
                what,
                name,
                node,
            } => {
                put_u(&mut self.body, *frame);
                put_u(&mut self.body, *what as u64);
                put_u(&mut self.body, *name as u64);
                put_u(&mut self.body, *node);
                tag::HOLDER
            }
            Record::FrameEnd {
                t,
                frame,
                unchanged,
                nodes,
            } => {
                self.t(*t);
                put_u(&mut self.body, *frame);
                put_u(
                    &mut self.body,
                    match unchanged {
                        None => 0,
                        Some(false) => 1,
                        Some(true) => 2,
                    },
                );
                put_u(&mut self.body, *nodes as u64);
                tag::FRAME_END
            }
            Record::Present {
                t,
                frame,
                render,
                wait,
            } => {
                self.t(*t);
                put_u(&mut self.body, *frame);
                put_u(&mut self.body, *render);
                put_u(&mut self.body, *wait);
                tag::PRESENT
            }
            Record::Span(s) => {
                self.t(s.start);
                put_u(&mut self.body, s.track.0 as u64);
                put_u(&mut self.body, s.id as u64);
                put_u(&mut self.body, s.parent as u64);
                put_u(&mut self.body, s.name as u64);
                match s.origin {
                    Origin::Kui => put_u(&mut self.body, 0),
                    Origin::Host => put_u(&mut self.body, 1),
                    Origin::Extension(ns) => put_u(&mut self.body, 2 + ns as u64),
                }
                put_u(&mut self.body, s.dur);
                put_opt(&mut self.body, s.cpu);
                put_u(&mut self.body, s.node);
                put_u(&mut self.body, s.cut as u64);
                tag::SPAN
            }
            Record::Input {
                t,
                kind,
                os_t,
                count,
                until,
            } => {
                self.t(*t);
                put_u(&mut self.body, *kind as u64);
                put_opt(&mut self.body, *os_t);
                put_u(&mut self.body, *count as u64);
                put_u(&mut self.body, until.saturating_sub(*t));
                tag::INPUT
            }
            Record::Event { t, kind, node } => {
                self.t(*t);
                put_u(&mut self.body, *kind as u64);
                put_u(&mut self.body, *node);
                tag::EVENT
            }
            Record::Wake { t, name } => {
                self.t(*t);
                put_u(&mut self.body, *name as u64);
                tag::WAKE
            }
            Record::Request { t, why, at } => {
                self.t(*t);
                put_u(&mut self.body, *why as u64);
                put_u(&mut self.body, *at as u64);
                tag::REQUEST
            }
            Record::Loop { t, event } => {
                self.t(*t);
                match event {
                    LoopEvent::Sleep { until } => {
                        put_u(&mut self.body, 0);
                        put_opt(&mut self.body, *until);
                    }
                    LoopEvent::Woke { by } => {
                        put_u(&mut self.body, 1);
                        put_u(&mut self.body, *by as u64);
                    }
                    LoopEvent::Os { kind } => {
                        put_u(&mut self.body, 2);
                        put_u(&mut self.body, *kind as u64);
                    }
                }
                tag::LOOP
            }
            Record::Counter {
                t,
                frame,
                name,
                value,
            } => {
                self.t(*t);
                put_u(&mut self.body, *frame);
                put_u(&mut self.body, *name as u64);
                put_i(&mut self.body, *value);
                tag::COUNTER
            }
            Record::Gauge { t, name, value } => {
                self.t(*t);
                put_u(&mut self.body, *name as u64);
                self.body.extend_from_slice(&value.to_le_bytes());
                tag::GAUGE
            }
            Record::Mark { t, track, name } => {
                self.t(*t);
                put_u(&mut self.body, track.0 as u64);
                put_u(&mut self.body, *name as u64);
                tag::MARK
            }
            Record::Note { t, text } => {
                self.t(*t);
                put_str(&mut self.body, text);
                tag::NOTE
            }
            Record::Placement {
                t,
                node,
                component,
                answer,
                why,
            } => {
                self.t(*t);
                put_u(&mut self.body, *node);
                put_u(&mut self.body, *component as u64);
                put_u(&mut self.body, *answer as u64);
                put_u(&mut self.body, *why as u64);
                tag::PLACEMENT
            }
        };
        out.push(tag);
        put_u(out, self.body.len() as u64);
        out.extend_from_slice(&self.body);
    }
}

/// The header every recording opens with.
pub fn header() -> Vec<u8> {
    let mut h = MAGIC.to_vec();
    h.extend_from_slice(&VERSION.to_le_bytes());
    h
}

/// Records written to `W` as they come — `KUI_PERF_TRACE`'s stream.
pub struct Writer<W: Write> {
    out: W,
    enc: Encoder,
    buf: Vec<u8>,
}

impl<W: Write> Writer<W> {
    /// A writer that has written the header.
    pub fn new(mut out: W) -> io::Result<Self> {
        out.write_all(&header())?;
        Ok(Writer {
            out,
            enc: Encoder::new(),
            buf: Vec::new(),
        })
    }

    pub fn write(&mut self, r: &Record) -> io::Result<()> {
        self.buf.clear();
        self.enc.encode(r, &mut self.buf);
        self.out.write_all(&self.buf)
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.out.flush()
    }

    pub fn into_inner(self) -> W {
        self.out
    }
}

struct Cursor<'a> {
    b: &'a [u8],
    at: usize,
}

impl Cursor<'_> {
    fn u(&mut self) -> Option<u64> {
        let mut v = 0u64;
        let mut shift = 0;
        loop {
            let byte = *self.b.get(self.at)?;
            self.at += 1;
            if shift < 64 {
                v |= ((byte & 0x7f) as u64) << shift;
            }
            if byte & 0x80 == 0 {
                return Some(v);
            }
            shift += 7;
        }
    }

    fn i(&mut self) -> Option<i64> {
        let u = self.u()?;
        Some(((u >> 1) as i64) ^ -((u & 1) as i64))
    }

    fn u32(&mut self) -> Option<u32> {
        self.u().map(|v| v as u32)
    }

    fn opt(&mut self) -> Option<Option<u64>> {
        self.u().map(|v| v.checked_sub(1))
    }

    fn bytes(&mut self, n: usize) -> Option<&[u8]> {
        let end = self.at.checked_add(n)?;
        let s = self.b.get(self.at..end)?;
        self.at = end;
        Some(s)
    }

    fn str(&mut self) -> Option<String> {
        let n = self.u()? as usize;
        self.bytes(n)
            .map(|b| String::from_utf8_lossy(b).into_owned())
    }

    fn f32(&mut self) -> Option<f32> {
        self.bytes(4)
            .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
    }

    fn f64(&mut self) -> Option<f64> {
        self.bytes(8)
            .map(|b| f64::from_le_bytes(b.try_into().unwrap()))
    }
}

/// Reads records back, in order.
pub struct Reader<'a> {
    b: &'a [u8],
    at: usize,
    dec: Decoder,
}

/// What decoding needs to carry from one record to the next: the last
/// time read, since times are written as deltas.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Decoder {
    prev_t: u64,
}

/// Where the record at `at` in `b` is: its tag, and its body's range; `None`
/// while `b` holds less than the whole of it.
pub(crate) fn framed(b: &[u8], at: usize) -> Option<(u8, std::ops::Range<usize>)> {
    let tag = *b.get(at)?;
    let mut c = Cursor { b, at: at + 1 };
    let len = c.u()? as usize;
    let end = c.at.checked_add(len)?;
    (end <= b.len()).then_some((tag, c.at..end))
}

/// Checks the header at the front of `b`: the bytes it takes, or why they
/// are not a recording; `Ok(None)` while fewer than a header's bytes came.
pub(crate) fn check_header(b: &[u8]) -> Result<Option<usize>, Error> {
    if b.len() < 10 {
        return if MAGIC.starts_with(&b[..b.len().min(8)]) {
            Ok(None)
        } else {
            Err(Error::NotATrace)
        };
    }
    if &b[..8] != MAGIC {
        return Err(Error::NotATrace);
    }
    let v = u16::from_le_bytes([b[8], b[9]]);
    if v != VERSION {
        return Err(Error::Version(v));
    }
    Ok(Some(10))
}

impl<'a> Reader<'a> {
    /// A reader past the header, or why the bytes are not a recording.
    pub fn new(b: &'a [u8]) -> Result<Self, Error> {
        let at = check_header(b)?.ok_or(Error::NotATrace)?;
        Ok(Reader {
            b,
            at,
            dec: Decoder::default(),
        })
    }
}

impl Decoder {
    fn t(&mut self, c: &mut Cursor<'_>) -> Option<u64> {
        let d = c.i()?;
        self.prev_t = (self.prev_t as i64).saturating_add(d).max(0) as u64;
        Some(self.prev_t)
    }

    /// The record a tag and its body make; `Some(None)` for a tag from a
    /// later version, skipped; `None` for a body shorter than its fields.
    pub(crate) fn decode(&mut self, tag: u8, body: &[u8]) -> Option<Option<Record>> {
        let mut c = Cursor { b: body, at: 0 };
        let r = match tag {
            tag::SESSION => Record::Session(Session {
                kui: c.str()?,
                app: c.str()?,
                platform: c.str()?,
                refresh_hz: c.f32()?,
                budget_ms: c.f32()?,
                scale: c.f32()?,
                wall_ms: c.u()?,
            }),
            tag::NAME => Record::Name {
                id: c.u32()?,
                text: c.str()?,
            },
            tag::TRACK => Record::Track {
                id: Track(c.u32()?),
                name: c.u32()?,
            },
            tag::FRAME_BEGIN => {
                let t = self.t(&mut c)?;
                let frame = c.u()?;
                let n = c.u()? as usize;
                let mut causes = Vec::with_capacity(n.min(64));
                for _ in 0..n {
                    causes.push(c.u32()?);
                }
                Record::FrameBegin { t, frame, causes }
            }
            tag::HOLDER => Record::Holder {
                frame: c.u()?,
                what: c.u32()?,
                name: c.u32()?,
                node: c.u()?,
            },
            tag::FRAME_END => Record::FrameEnd {
                t: self.t(&mut c)?,
                frame: c.u()?,
                unchanged: match c.u()? {
                    1 => Some(false),
                    2 => Some(true),
                    _ => None,
                },
                nodes: c.u32()?,
            },
            tag::PRESENT => Record::Present {
                t: self.t(&mut c)?,
                frame: c.u()?,
                render: c.u()?,
                wait: c.u()?,
            },
            tag::SPAN => {
                let start = self.t(&mut c)?;
                Record::Span(Span {
                    start,
                    track: Track(c.u32()?),
                    id: c.u32()?,
                    parent: c.u32()?,
                    name: c.u32()?,
                    origin: match c.u()? {
                        0 => Origin::Kui,
                        1 => Origin::Host,
                        n => Origin::Extension((n - 2) as NameId),
                    },
                    dur: c.u()?,
                    cpu: c.opt()?,
                    node: c.u()?,
                    cut: c.u()? != 0,
                })
            }
            tag::INPUT => {
                let t = self.t(&mut c)?;
                Record::Input {
                    t,
                    kind: c.u32()?,
                    os_t: c.opt()?,
                    count: c.u32()?,
                    until: t.saturating_add(c.u()?),
                }
            }
            tag::EVENT => Record::Event {
                t: self.t(&mut c)?,
                kind: c.u32()?,
                node: c.u()?,
            },
            tag::WAKE => Record::Wake {
                t: self.t(&mut c)?,
                name: c.u32()?,
            },
            tag::REQUEST => Record::Request {
                t: self.t(&mut c)?,
                why: c.u32()?,
                at: c.u32()?,
            },
            tag::LOOP => {
                let t = self.t(&mut c)?;
                let event = match c.u()? {
                    0 => LoopEvent::Sleep { until: c.opt()? },
                    1 => LoopEvent::Woke { by: c.u32()? },
                    _ => LoopEvent::Os { kind: c.u32()? },
                };
                Record::Loop { t, event }
            }
            tag::COUNTER => Record::Counter {
                t: self.t(&mut c)?,
                frame: c.u()?,
                name: c.u32()?,
                value: c.i()?,
            },
            tag::GAUGE => Record::Gauge {
                t: self.t(&mut c)?,
                name: c.u32()?,
                value: c.f64()?,
            },
            tag::MARK => Record::Mark {
                t: self.t(&mut c)?,
                track: Track(c.u32()?),
                name: c.u32()?,
            },
            tag::NOTE => Record::Note {
                t: self.t(&mut c)?,
                text: c.str()?,
            },
            tag::PLACEMENT => Record::Placement {
                t: self.t(&mut c)?,
                node: c.u()?,
                component: c.u32()?,
                answer: c.u32()?,
                why: c.u32()?,
            },
            // A tag from a later version: skipped by its length.
            _ => return Some(None),
        };
        Some(Some(r))
    }
}

impl Iterator for Reader<'_> {
    type Item = Record;

    /// The next record; `None` at the end, or where a recording cut off
    /// mid-record stops.
    fn next(&mut self) -> Option<Record> {
        loop {
            let (tag, body) = framed(self.b, self.at)?;
            self.at = body.end;
            match self.dec.decode(tag, &self.b[body]) {
                Some(Some(r)) => return Some(r),
                // Unknown, or a body shorter than its fields: skipped.
                Some(None) | None => continue,
            }
        }
    }
}
