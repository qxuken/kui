//! Perfetto's protobuf trace, with the encoder written for the few
//! messages it needs (field numbers from `protos/perfetto/trace/`:
//! `trace.proto`, `trace_packet.proto`, `track_event/track_event.proto`,
//! `track_descriptor.proto`, `counter_descriptor.proto`,
//! `process_descriptor.proto`, `thread_descriptor.proto`,
//! `debug_annotation.proto`).
//!
//! A `Trace` is `TracePacket`s (field 1), all on one sequence. The first
//! describes the process (the app, pid 1) and clears the sequence's
//! incremental state; then a `TrackDescriptor` a lane — `frames` a track
//! under the process, each track a thread of it (tid as Chrome's) — and a
//! counter track a series. Then `TrackEvent`s in time order: a slice as a
//! begin and an end, an instant, a counter's value; names and categories
//! written out, never interned, so no packet needs incremental state.
//! Arguments are debug annotations. Times are ns on the default clock:
//! the recording's µs × 1000.

use super::{Model, Val};

/// Field numbers and enum values, by message.
mod f {
    // Trace
    pub const PACKET: u32 = 1;
    // TracePacket
    pub const TIMESTAMP: u32 = 8;
    pub const SEQUENCE_ID: u32 = 10;
    pub const TRACK_EVENT: u32 = 11;
    pub const SEQUENCE_FLAGS: u32 = 13;
    pub const TRACK_DESCRIPTOR: u32 = 60;
    pub const SEQ_INCREMENTAL_STATE_CLEARED: u64 = 1;
    // TrackDescriptor
    pub const UUID: u32 = 1;
    pub const NAME: u32 = 2;
    pub const PROCESS: u32 = 3;
    pub const THREAD: u32 = 4;
    pub const PARENT_UUID: u32 = 5;
    pub const COUNTER: u32 = 8;
    // ProcessDescriptor
    pub const PD_PID: u32 = 1;
    pub const PD_NAME: u32 = 6;
    // ThreadDescriptor
    pub const TD_PID: u32 = 1;
    pub const TD_TID: u32 = 2;
    pub const TD_NAME: u32 = 5;
    // TrackEvent
    pub const DEBUG_ANNOTATIONS: u32 = 4;
    pub const TYPE: u32 = 9;
    pub const TRACK_UUID: u32 = 11;
    pub const CATEGORIES: u32 = 22;
    pub const EV_NAME: u32 = 23;
    pub const COUNTER_VALUE: u32 = 30;
    pub const DOUBLE_COUNTER_VALUE: u32 = 44;
    pub const TYPE_SLICE_BEGIN: u64 = 1;
    pub const TYPE_SLICE_END: u64 = 2;
    pub const TYPE_INSTANT: u64 = 3;
    pub const TYPE_COUNTER: u64 = 4;
    // DebugAnnotation
    pub const DA_BOOL: u32 = 2;
    pub const DA_UINT: u32 = 3;
    pub const DA_INT: u32 = 4;
    pub const DA_DOUBLE: u32 = 5;
    pub const DA_STRING: u32 = 6;
    pub const DA_NAME: u32 = 10;
}

/// The sequence every packet is on.
const SEQUENCE: u64 = 1;
const PROCESS_UUID: u64 = 1;
const PID: u64 = 1;

fn lane_uuid(i: usize) -> u64 {
    0x100 + i as u64
}

fn series_uuid(i: usize) -> u64 {
    0x1_0000 + i as u64
}

/// A message being written: fields as protobuf's wire format has them.
#[derive(Default)]
pub(crate) struct Msg(pub Vec<u8>);

impl Msg {
    fn key(&mut self, field: u32, wire: u8) {
        self.raw_varint(((field as u64) << 3) | wire as u64);
    }

    fn raw_varint(&mut self, mut v: u64) {
        loop {
            let b = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                self.0.push(b);
                return;
            }
            self.0.push(b | 0x80);
        }
    }

    pub fn varint(&mut self, field: u32, v: u64) -> &mut Self {
        self.key(field, 0);
        self.raw_varint(v);
        self
    }

    /// An `int64`: two's complement, ten bytes when negative.
    pub fn int64(&mut self, field: u32, v: i64) -> &mut Self {
        self.varint(field, v as u64)
    }

    pub fn double(&mut self, field: u32, v: f64) -> &mut Self {
        self.key(field, 1);
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }

    pub fn bytes(&mut self, field: u32, b: &[u8]) -> &mut Self {
        self.key(field, 2);
        self.raw_varint(b.len() as u64);
        self.0.extend_from_slice(b);
        self
    }

    pub fn str(&mut self, field: u32, s: &str) -> &mut Self {
        self.bytes(field, s.as_bytes())
    }

    pub fn msg(&mut self, field: u32, m: &Msg) -> &mut Self {
        self.bytes(field, &m.0)
    }
}

fn annotations(ev: &mut Msg, args: &[(&str, Val)]) {
    for (k, v) in args {
        let mut a = Msg::default();
        a.str(f::DA_NAME, k);
        match v {
            Val::Str(s) => a.str(f::DA_STRING, s),
            Val::U(u) => a.varint(f::DA_UINT, *u),
            Val::I(i) => a.int64(f::DA_INT, *i),
            Val::F(x) => a.double(f::DA_DOUBLE, *x),
            Val::B(b) => a.varint(f::DA_BOOL, *b as u64),
        };
        ev.msg(f::DEBUG_ANNOTATIONS, &a);
    }
}

fn packet(out: &mut Msg, p: &Msg) {
    out.msg(f::PACKET, p);
}

fn descriptor(out: &mut Msg, d: &Msg, first: bool) {
    let mut p = Msg::default();
    p.msg(f::TRACK_DESCRIPTOR, d);
    p.varint(f::SEQUENCE_ID, SEQUENCE);
    if first {
        p.varint(f::SEQUENCE_FLAGS, f::SEQ_INCREMENTAL_STATE_CLEARED);
    }
    packet(out, &p);
}

pub fn write(m: &Model) -> Vec<u8> {
    let mut out = Msg::default();

    let mut pd = Msg::default();
    pd.varint(f::PD_PID, PID).str(f::PD_NAME, &m.app);
    let mut d = Msg::default();
    d.varint(f::UUID, PROCESS_UUID).msg(f::PROCESS, &pd);
    descriptor(&mut out, &d, true);

    for (i, lane) in m.lanes.iter().enumerate() {
        let mut d = Msg::default();
        d.varint(f::UUID, lane_uuid(i));
        if lane.track.is_some() {
            let mut td = Msg::default();
            td.varint(f::TD_PID, PID)
                .varint(f::TD_TID, lane.tid)
                .str(f::TD_NAME, &lane.name);
            d.msg(f::THREAD, &td);
        } else {
            d.varint(f::PARENT_UUID, PROCESS_UUID)
                .str(f::NAME, &lane.name);
        }
        descriptor(&mut out, &d, false);
    }
    for (i, s) in m.series.iter().enumerate() {
        let mut d = Msg::default();
        d.varint(f::UUID, series_uuid(i))
            .varint(f::PARENT_UUID, PROCESS_UUID)
            .str(f::NAME, &s.name)
            .msg(f::COUNTER, &Msg::default());
        descriptor(&mut out, &d, false);
    }

    // Every event with its time; each lane's in its own order, which a
    // stable sort by time keeps.
    let mut events: Vec<(u64, Msg)> = Vec::new();
    for lane in 0..m.lanes.len() {
        for (at, opens, i) in m.edges(lane) {
            let s = &m.slices[i];
            let mut ev = Msg::default();
            ev.varint(f::TRACK_UUID, lane_uuid(lane));
            if opens {
                ev.varint(f::TYPE, f::TYPE_SLICE_BEGIN)
                    .str(f::EV_NAME, &s.label)
                    .str(f::CATEGORIES, &s.cat);
                annotations(&mut ev, &s.args);
            } else {
                ev.varint(f::TYPE, f::TYPE_SLICE_END);
            }
            events.push((at, ev));
        }
    }
    for i in &m.instants {
        let mut ev = Msg::default();
        ev.varint(f::TRACK_UUID, lane_uuid(i.lane))
            .varint(f::TYPE, f::TYPE_INSTANT)
            .str(f::EV_NAME, &i.name)
            .str(f::CATEGORIES, i.cat);
        annotations(&mut ev, &i.args);
        events.push((i.t, ev));
    }
    for (si, s) in m.series.iter().enumerate() {
        for (t, v) in &s.points {
            let mut ev = Msg::default();
            ev.varint(f::TRACK_UUID, series_uuid(si))
                .varint(f::TYPE, f::TYPE_COUNTER);
            match v {
                Val::I(i) => ev.int64(f::COUNTER_VALUE, *i),
                Val::U(u) => ev.int64(f::COUNTER_VALUE, *u as i64),
                Val::F(x) => ev.double(f::DOUBLE_COUNTER_VALUE, *x),
                Val::B(b) => ev.int64(f::COUNTER_VALUE, *b as i64),
                Val::Str(_) => continue,
            };
            events.push((*t, ev));
        }
    }
    events.sort_by_key(|e| e.0);
    for (t, ev) in &events {
        let mut p = Msg::default();
        p.varint(f::TIMESTAMP, t.saturating_mul(1000))
            .varint(f::SEQUENCE_ID, SEQUENCE)
            .msg(f::TRACK_EVENT, ev);
        packet(&mut out, &p);
    }
    out.0
}
