//! A recording read as it arrives (backlog F246, ADR 0075 decision 5):
//! bytes pushed in as a [`Transport`] hands them over — a file still being
//! written, a pipe, a socket — decoded up to the last whole record, the
//! cut one kept for the next push, the recording and its timeline grown.
//!
//! ```
//! use kui_trace::{Record, Stream, Trace};
//!
//! let mut t = Trace::new();
//! t.records.push(Record::FrameBegin { t: 0, frame: 1, causes: vec![] });
//! t.records.push(Record::FrameEnd { t: 900, frame: 1, unchanged: Some(false), nodes: 3 });
//! let bytes = t.to_bytes();
//!
//! let mut s = Stream::new();
//! let (head, rest) = bytes.split_at(bytes.len() - 2);
//! s.push(head)?;
//! assert_eq!(s.timeline().frames[0].end, None, "the end is cut");
//! s.push(rest)?;
//! assert_eq!(s.timeline().frames[0].end, Some(900));
//! # Ok::<(), kui_trace::Error>(())
//! ```

use std::io;

use crate::codec::{Decoder, Error, check_header, framed};
use crate::model::{Timeline, Trace};

/// Bytes consumed past this are dropped from the front of the buffer.
const COMPACT_PAST: usize = 1 << 16;

/// A recording decoded as its bytes come.
#[derive(Default)]
pub struct Stream {
    buf: Vec<u8>,
    at: usize,
    header: bool,
    dec: Decoder,
    trace: Trace,
    timeline: Timeline,
}

impl Stream {
    pub fn new() -> Self {
        Stream {
            trace: Trace::new(),
            ..Default::default()
        }
    }

    /// Decodes what `bytes` complete: how many records came. A cut record
    /// waits for the next push; bytes that are no recording are an error,
    /// and so is a header of another version.
    pub fn push(&mut self, bytes: &[u8]) -> Result<usize, Error> {
        self.buf.extend_from_slice(bytes);
        if !self.header {
            match check_header(&self.buf)? {
                Some(n) => {
                    self.at = n;
                    self.header = true;
                }
                None => return Ok(0),
            }
        }
        let from = self.trace.records.len();
        let mut came = 0;
        while let Some((tag, body)) = framed(&self.buf, self.at) {
            self.at = body.end;
            // A body shorter than its fields is skipped, as a reader of a
            // whole file skips it.
            if let Some(Some(r)) = self.dec.decode(tag, &self.buf[body]) {
                self.trace.push(r);
                came += 1;
            }
        }
        self.timeline.extend(&self.trace.records[from..]);
        if self.at > COMPACT_PAST {
            self.buf.drain(..self.at);
            self.at = 0;
        }
        Ok(came)
    }

    /// Starts again, as a source that began a new recording asks.
    pub fn reset(&mut self) {
        *self = Stream::new();
    }

    /// The recording so far.
    pub fn trace(&self) -> &Trace {
        &self.trace
    }

    /// The timeline so far.
    pub fn timeline(&self) -> &Timeline {
        &self.timeline
    }

    /// Whether the header has come.
    pub fn started(&self) -> bool {
        self.header
    }

    /// The bytes of a record cut short, waiting for the rest.
    pub fn pending(&self) -> usize {
        self.buf.len() - self.at
    }

    /// Reads what `t` has into the stream: [`Polled::More`] with the
    /// records that came, [`Polled::Idle`], [`Polled::Ended`], or
    /// [`Polled::Reset`] after starting again for a new recording.
    pub fn poll(&mut self, t: &mut dyn Transport) -> Result<Polled, PollError> {
        let mut buf = Vec::new();
        match t.poll(&mut buf).map_err(PollError::Io)? {
            Polled::Reset => {
                self.reset();
                self.push(&buf).map_err(PollError::Trace)?;
                Ok(Polled::Reset)
            }
            Polled::More(_) => Ok(Polled::More(self.push(&buf).map_err(PollError::Trace)?)),
            other => Ok(other),
        }
    }
}

/// What a [`Transport`] had.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Polled {
    /// Bytes, appended to the buffer: how many (from [`Stream::poll`], how
    /// many records they completed).
    More(usize),
    /// Nothing yet; more may come.
    Idle,
    /// Nothing more will come.
    Ended,
    /// The source began again — a file rewritten, a run restarted: what
    /// was read is another recording's, and the buffer holds the new one
    /// from its start.
    Reset,
}

/// Where a recording's bytes come from (ADR 0075, decision 5): a file
/// ([`FileTransport`]), and later a pipe or a socket, behind one door.
pub trait Transport: Send {
    /// Appends what came since the last poll to `buf`, without waiting.
    fn poll(&mut self, buf: &mut Vec<u8>) -> io::Result<Polled>;
}

/// Why [`Stream::poll`] stopped.
#[derive(Debug)]
pub enum PollError {
    Io(io::Error),
    Trace(Error),
}

impl std::fmt::Display for PollError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PollError::Io(e) => write!(f, "{e}"),
            PollError::Trace(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for PollError {}
