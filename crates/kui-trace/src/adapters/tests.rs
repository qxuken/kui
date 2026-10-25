//! Each adapter's output read back: JSON by a strict parser written
//! here, Perfetto's protobuf by a decoder written here, and the structure
//! each format needs checked — slices nested per row, opens and closes
//! balanced, names and arguments where they belong.

use std::collections::HashMap;

use super::*;
use crate::{LoopEvent, Origin, Record, Session, Span, Trace, Track};

const NOTE: &str = "she said \"hi\"\nthen a \\ and a\ttab, \u{1} é ✓ \u{2028}";

fn span(id: u32, parent: u32, name: u32, track: Track, start: u64, dur: u64) -> Span {
    Span {
        track,
        id,
        parent,
        name,
        origin: Origin::Kui,
        start,
        dur,
        cpu: Some(dur / 2),
        node: 0,
        cut: false,
    }
}

/// Three frames (the last cut off), nested spans on `main` (one running
/// past its parent), a span on a declared track, the loop asleep, an
/// input, an event, a wake, a request, a mark, counters, a gauge and a
/// note with a quote and a newline.
fn sample() -> Trace {
    let mut t = Trace::new();
    t.session = Some(Session {
        kui: "0.1.0".into(),
        app: "demo \"app\"".into(),
        platform: "macos".into(),
        refresh_hz: 120.0,
        budget_ms: 8.33,
        scale: 2.0,
        wall_ms: 1_761_000_000_000,
    });
    let mut n = |s: &str| t.names.get(s).0;
    let (key, wake, build, rows, layout, overrun, parse, parser, chunk, laid, memory) = (
        n("key"),
        n("wake"),
        n("build"),
        n("rows"),
        n("layout"),
        n("overrun"),
        n("parse"),
        n("parser"),
        n("chunk"),
        n("laid"),
        n("memory"),
    );
    let (click, blink, at, input, owed) = (
        n("click"),
        n("blink"),
        n("src/app.rs:4"),
        n("input"),
        n("owed"),
    );
    t.tracks.push((Track(2), parser));
    let mut rows_span = span(2, 1, rows, Track::MAIN, 100, 500);
    rows_span.origin = Origin::Host;
    rows_span.node = 0xabc;
    t.records = vec![
        Record::FrameBegin {
            t: 0,
            frame: 1,
            causes: vec![key, wake],
        },
        Record::Holder {
            frame: 1,
            what: wake,
            name: parser,
            node: 7,
        },
        Record::Span(rows_span),
        Record::Span(span(4, 1, overrun, Track::MAIN, 900, 400)),
        Record::Span(span(1, 0, build, Track::MAIN, 10, 1000)),
        Record::Span(span(3, 0, layout, Track::MAIN, 1010, 490)),
        Record::Span(span(5, 0, parse, Track(2), 200, 5000)),
        Record::Mark {
            t: 3000,
            track: Track(2),
            name: chunk,
        },
        Record::Gauge {
            t: 5000,
            name: memory,
            value: 1.5e6,
        },
        Record::Counter {
            t: 1600,
            frame: 1,
            name: laid,
            value: 40,
        },
        Record::FrameEnd {
            t: 1600,
            frame: 1,
            unchanged: Some(false),
            nodes: 12,
        },
        Record::Present {
            t: 9000,
            frame: 1,
            render: 2000,
            wait: 5000,
        },
        Record::Loop {
            t: 10_000,
            event: LoopEvent::Sleep {
                until: Some(16_000),
            },
        },
        Record::Loop {
            t: 11_900,
            event: LoopEvent::Woke { by: input },
        },
        Record::Input {
            t: 12_000,
            kind: key,
            os_t: Some(11_950),
            count: 1,
            until: 12_000,
        },
        Record::Event {
            t: 12_500,
            kind: click,
            node: 5,
        },
        Record::Wake {
            t: 13_000,
            name: parser,
        },
        Record::Request {
            t: 13_500,
            why: blink,
            at,
        },
        Record::Note {
            t: 14_000,
            text: NOTE.into(),
        },
        Record::FrameBegin {
            t: 16_000,
            frame: 2,
            causes: vec![key],
        },
        Record::Span(span(6, 0, build, Track::MAIN, 16_010, 700)),
        Record::Counter {
            t: 17_000,
            frame: 2,
            name: laid,
            value: -3,
        },
        Record::FrameEnd {
            t: 17_000,
            frame: 2,
            unchanged: Some(true),
            nodes: 12,
        },
        Record::FrameBegin {
            t: 32_000,
            frame: 3,
            causes: vec![owed],
        },
        Record::Span(span(7, 0, build, Track::MAIN, 32_010, 300)),
    ];
    // Through the bytes, as a tool reads it.
    Trace::from_bytes(&t.to_bytes()).unwrap()
}

// --- a strict JSON parser -------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    fn get(&self, k: &str) -> &Json {
        match self {
            Json::Obj(kv) => kv
                .iter()
                .find(|(key, _)| key == k)
                .map(|(_, v)| v)
                .unwrap_or_else(|| panic!("no key {k}")),
            j => panic!("{k} of a non-object {j:?}"),
        }
    }

    fn has(&self, k: &str) -> bool {
        matches!(self, Json::Obj(kv) if kv.iter().any(|(key, _)| key == k))
    }

    fn arr(&self) -> &[Json] {
        match self {
            Json::Arr(a) => a,
            j => panic!("not an array: {j:?}"),
        }
    }

    fn str(&self) -> &str {
        match self {
            Json::Str(s) => s,
            j => panic!("not a string: {j:?}"),
        }
    }

    fn num(&self) -> f64 {
        match self {
            Json::Num(n) => *n,
            j => panic!("not a number: {j:?}"),
        }
    }
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
}

fn parse_json(s: &str) -> Result<Json, String> {
    let mut p = Parser {
        b: s.as_bytes(),
        i: 0,
    };
    let v = p.value()?;
    p.ws();
    if p.i != p.b.len() {
        return Err(format!("trailing bytes at {}", p.i));
    }
    Ok(v)
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\n' | b'\r' | b'\t') {
            self.i += 1;
        }
    }

    fn eat(&mut self, c: u8) -> Result<(), String> {
        self.ws();
        if self.b.get(self.i) == Some(&c) {
            self.i += 1;
            Ok(())
        } else {
            Err(format!("expected {:?} at {}", c as char, self.i))
        }
    }

    fn lit(&mut self, s: &str, v: Json) -> Result<Json, String> {
        if self.b[self.i..].starts_with(s.as_bytes()) {
            self.i += s.len();
            Ok(v)
        } else {
            Err(format!("bad literal at {}", self.i))
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        self.ws();
        match self.b.get(self.i) {
            Some(b'{') => {
                self.i += 1;
                let mut kv = Vec::new();
                self.ws();
                if self.b.get(self.i) == Some(&b'}') {
                    self.i += 1;
                    return Ok(Json::Obj(kv));
                }
                loop {
                    self.ws();
                    let k = self.string()?;
                    self.eat(b':')?;
                    let v = self.value()?;
                    if kv.iter().any(|(key, _)| *key == k) {
                        return Err(format!("duplicate key {k}"));
                    }
                    kv.push((k, v));
                    self.ws();
                    match self.b.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b'}') => {
                            self.i += 1;
                            return Ok(Json::Obj(kv));
                        }
                        _ => return Err(format!("bad object at {}", self.i)),
                    }
                }
            }
            Some(b'[') => {
                self.i += 1;
                let mut a = Vec::new();
                self.ws();
                if self.b.get(self.i) == Some(&b']') {
                    self.i += 1;
                    return Ok(Json::Arr(a));
                }
                loop {
                    a.push(self.value()?);
                    self.ws();
                    match self.b.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b']') => {
                            self.i += 1;
                            return Ok(Json::Arr(a));
                        }
                        _ => return Err(format!("bad array at {}", self.i)),
                    }
                }
            }
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b't') => self.lit("true", Json::Bool(true)),
            Some(b'f') => self.lit("false", Json::Bool(false)),
            Some(b'n') => self.lit("null", Json::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(format!("bad value at {}", self.i)),
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.i;
        let digits = |p: &mut Self| {
            let s = p.i;
            while p.i < p.b.len() && p.b[p.i].is_ascii_digit() {
                p.i += 1;
            }
            p.i - s
        };
        if self.b[self.i] == b'-' {
            self.i += 1;
        }
        let int_start = self.i;
        if digits(self) == 0 {
            return Err(format!("bad number at {start}"));
        }
        if self.b[int_start] == b'0' && self.i - int_start > 1 {
            return Err(format!("leading zero at {start}"));
        }
        if self.b.get(self.i) == Some(&b'.') {
            self.i += 1;
            if digits(self) == 0 {
                return Err(format!("bad fraction at {start}"));
            }
        }
        if matches!(self.b.get(self.i), Some(b'e' | b'E')) {
            self.i += 1;
            if matches!(self.b.get(self.i), Some(b'+' | b'-')) {
                self.i += 1;
            }
            if digits(self) == 0 {
                return Err(format!("bad exponent at {start}"));
            }
        }
        let s = std::str::from_utf8(&self.b[start..self.i]).unwrap();
        s.parse().map(Json::Num).map_err(|e| format!("{s}: {e}"))
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let h = self.b.get(self.i..self.i + 4).ok_or("short \\u escape")?;
        self.i += 4;
        u32::from_str_radix(std::str::from_utf8(h).map_err(|e| e.to_string())?, 16)
            .map_err(|e| e.to_string())
    }

    fn string(&mut self) -> Result<String, String> {
        if self.b.get(self.i) != Some(&b'"') {
            return Err(format!("expected a string at {}", self.i));
        }
        self.i += 1;
        let mut out = String::new();
        loop {
            let c = *self.b.get(self.i).ok_or("unterminated string")?;
            match c {
                b'"' => {
                    self.i += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.i += 1;
                    let e = *self.b.get(self.i).ok_or("unterminated escape")?;
                    self.i += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let mut u = self.hex4()?;
                            if (0xd800..0xdc00).contains(&u) {
                                if !self.b[self.i..].starts_with(b"\\u") {
                                    return Err("lone surrogate".into());
                                }
                                self.i += 2;
                                let lo = self.hex4()?;
                                u = 0x10000 + ((u - 0xd800) << 10) + (lo - 0xdc00);
                            }
                            out.push(char::from_u32(u).ok_or("bad code point")?);
                        }
                        _ => return Err(format!("bad escape at {}", self.i)),
                    }
                }
                c if c < 0x20 => return Err(format!("raw control character at {}", self.i)),
                _ => {
                    // One UTF-8 character, whole.
                    let rest = std::str::from_utf8(&self.b[self.i..]).map_err(|e| e.to_string());
                    let rest = match rest {
                        Ok(r) => r,
                        Err(_) => std::str::from_utf8(&self.b[self.i..self.i + 1])
                            .map_err(|e| e.to_string())?,
                    };
                    let ch = rest.chars().next().unwrap();
                    out.push(ch);
                    self.i += ch.len_utf8();
                }
            }
        }
    }
}

#[test]
fn the_json_parser_is_strict() {
    assert!(parse_json(r#"{"a":[1,2.5,-3e2,true,null,"x\n\u00e9\ud83d\ude00"]}"#).is_ok());
    for bad in [
        "{",
        "[1,]",
        "{\"a\":1,}",
        "\"\n\"",
        "01",
        "1.",
        "{\"a\" 1}",
        "\"\\x\"",
        "[1] 2",
        "{\"a\":1,\"a\":2}",
    ] {
        assert!(parse_json(bad).is_err(), "{bad:?} parsed");
    }
}

#[test]
fn json_strings_escape_what_json_needs() {
    let mut s = String::new();
    json::string(&mut s, NOTE);
    assert!(!s.bytes().any(|b| b < 0x20), "{s}");
    assert_eq!(parse_json(&s).unwrap(), Json::Str(NOTE.into()));
    let mut n = String::new();
    json::number(&mut n, f64::NAN);
    assert_eq!(n, "null");
}

// --- the model ------------------------------------------------------------

#[test]
fn the_model_nests_each_lane_and_places_the_present() {
    let t = sample();
    let m = Model::of(&t);
    let names: Vec<&str> = m.lanes.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["frames", "main", "loop", "parser"]);
    let tids: Vec<u64> = m.lanes.iter().map(|l| l.tid).collect();
    assert_eq!(tids, [4, 1, 2, 3]);
    let find = |label: &str| m.slices.iter().find(|s| s.label == label).unwrap();
    assert_eq!(
        (find("overrun").start, find("overrun").end),
        (900, 1010),
        "cut to build"
    );
    assert_eq!((find("render").start, find("render").end), (2000, 4000));
    assert_eq!(
        (find("vsync wait").start, find("vsync wait").end),
        (4000, 9000)
    );
    assert_eq!((find("asleep").start, find("asleep").end), (10_000, 11_900));
    let f3 = find("frame 3");
    assert_eq!(f3.end, 32_310, "cut off: to the recording's end");
    assert!(f3.args.contains(&("unfinished", Val::B(true))));
    for lane in 0..m.lanes.len() {
        let mut stack: Vec<usize> = Vec::new();
        let mut last = 0;
        for (at, opens, i) in m.edges(lane) {
            assert!(at >= last, "edges in order");
            last = at;
            if opens {
                stack.push(i);
            } else {
                assert_eq!(stack.pop(), Some(i), "a close matches its open");
            }
        }
        assert!(stack.is_empty());
    }
}

// --- Chrome ---------------------------------------------------------------

#[test]
fn chrome_is_trace_events_nested_per_thread() {
    let t = sample();
    let out = chrome(&t);
    let j = parse_json(&out).unwrap_or_else(|e| panic!("{e}\n{out}"));
    assert_eq!(j.get("displayTimeUnit").str(), "ms");
    assert_eq!(j.get("otherData").get("app").str(), "demo \"app\"");
    let evs = j.get("traceEvents").arr();
    let ph = |e: &Json| e.get("ph").str().to_string();

    // The threads, named.
    let mut tid_of: HashMap<String, f64> = HashMap::new();
    for e in evs.iter().filter(|e| ph(e) == "M") {
        match e.get("name").str() {
            "thread_name" => {
                tid_of.insert(
                    e.get("args").get("name").str().to_string(),
                    e.get("tid").num(),
                );
            }
            "process_name" => assert_eq!(e.get("args").get("name").str(), "demo \"app\""),
            "thread_sort_index" => {}
            n => panic!("metadata {n}"),
        }
    }
    assert_eq!(tid_of["main"], 1.0);
    assert_eq!(tid_of["loop"], 2.0);
    assert_eq!(tid_of["parser"], 3.0);
    assert_eq!(tid_of["frames"], 4.0);

    let x = |name: &str| {
        evs.iter()
            .find(|e| ph(e) == "X" && e.get("name").str() == name)
            .unwrap_or_else(|| panic!("no X {name}"))
    };
    assert_eq!(x("frame 1").get("tid").num(), 4.0);
    assert_eq!(x("frame 1").get("args").get("causes").str(), "key, wake");
    assert_eq!(
        x("frame 1").get("args").get("holders").str(),
        "wake parser node 0000000000000007"
    );
    assert_eq!(x("build").get("tid").num(), 1.0);
    assert_eq!(x("rows").get("cat").str(), "host");
    assert_eq!(x("rows").get("args").get("node").str(), "0000000000000abc");
    assert_eq!(
        x("build").get("args").get("self_us").num(),
        390.0,
        "less rows, and the overrun up to build's end"
    );
    assert_eq!(x("parse").get("tid").num(), 3.0);
    assert_eq!(
        (x("overrun").get("ts").num(), x("overrun").get("dur").num()),
        (900.0, 110.0)
    );
    assert_eq!(x("render").get("ts").num(), 2000.0);

    // X events nest per thread.
    let mut by_tid: HashMap<u64, Vec<(f64, f64)>> = HashMap::new();
    for e in evs.iter().filter(|e| ph(e) == "X") {
        by_tid
            .entry(e.get("tid").num() as u64)
            .or_default()
            .push((e.get("ts").num(), e.get("ts").num() + e.get("dur").num()));
    }
    for (tid, mut v) in by_tid {
        v.sort_by(|a, b| a.0.total_cmp(&b.0).then(b.1.total_cmp(&a.1)));
        let mut stack: Vec<f64> = Vec::new();
        for (a, b) in v {
            while stack.last().is_some_and(|&end| end <= a) {
                stack.pop();
            }
            if let Some(&end) = stack.last() {
                assert!(
                    b <= end,
                    "thread {tid}: [{a}, {b}] crosses its parent's end {end}"
                );
            }
            stack.push(b);
        }
    }

    // Instants, counters, the note's text whole.
    let inst = |name: &str| {
        evs.iter()
            .find(|e| ph(e) == "i" && e.get("name").str() == name)
            .unwrap_or_else(|| panic!("no instant {name}"))
    };
    assert_eq!(inst("input key").get("tid").num(), 1.0);
    assert_eq!(inst("input key").get("s").str(), "t");
    assert_eq!(
        inst("event click").get("args").get("node").str(),
        "0000000000000005"
    );
    assert_eq!(inst("chunk").get("tid").num(), 3.0);
    assert_eq!(
        inst("request blink").get("args").get("at").str(),
        "src/app.rs:4"
    );
    assert_eq!(inst("woke input").get("tid").num(), 2.0);
    let note = inst("note");
    assert_eq!(note.get("s").str(), "g");
    assert_eq!(note.get("args").get("text").str(), NOTE);
    let counters: Vec<f64> = evs
        .iter()
        .filter(|e| ph(e) == "C" && e.get("name").str() == "laid")
        .map(|e| e.get("args").get("laid").num())
        .collect();
    // A step over each frame it moved in: its value, then 0 at the end.
    assert_eq!(counters, [40.0, 0.0, -3.0, 0.0]);
    let gauge = evs
        .iter()
        .find(|e| ph(e) == "C" && e.get("name").str() == "memory")
        .unwrap();
    assert_eq!(gauge.get("args").get("memory").num(), 1.5e6);
    // Sorted by time after the metadata.
    let ts: Vec<f64> = evs
        .iter()
        .filter(|e| ph(e) != "M")
        .map(|e| e.get("ts").num())
        .collect();
    assert!(ts.windows(2).all(|w| w[0] <= w[1]));
}

// --- Perfetto -------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum Wire {
    Varint(u64),
    Fixed64(u64),
    Bytes(Vec<u8>),
    Fixed32(u32),
}

/// A message's fields in order.
fn decode(b: &[u8]) -> Vec<(u32, Wire)> {
    fn varint(b: &[u8], i: &mut usize) -> u64 {
        let mut v = 0u64;
        let mut shift = 0;
        loop {
            let byte = b[*i];
            *i += 1;
            v |= ((byte & 0x7f) as u64) << shift;
            if byte & 0x80 == 0 {
                return v;
            }
            shift += 7;
            assert!(shift < 64, "varint too long");
        }
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let key = varint(b, &mut i);
        let (field, wire) = ((key >> 3) as u32, key & 7);
        assert!(field > 0, "field 0");
        let v = match wire {
            0 => Wire::Varint(varint(b, &mut i)),
            1 => {
                let v = u64::from_le_bytes(b[i..i + 8].try_into().unwrap());
                i += 8;
                Wire::Fixed64(v)
            }
            2 => {
                let n = varint(b, &mut i) as usize;
                let v = b[i..i + n].to_vec();
                i += n;
                Wire::Bytes(v)
            }
            5 => {
                let v = u32::from_le_bytes(b[i..i + 4].try_into().unwrap());
                i += 4;
                Wire::Fixed32(v)
            }
            w => panic!("wire type {w}"),
        };
        out.push((field, v));
    }
    assert_eq!(i, b.len(), "a message ends on a field");
    out
}

fn field(m: &[(u32, Wire)], f: u32) -> Option<&Wire> {
    m.iter().find(|(n, _)| *n == f).map(|(_, w)| w)
}

fn sub(m: &[(u32, Wire)], f: u32) -> Option<Vec<(u32, Wire)>> {
    match field(m, f)? {
        Wire::Bytes(b) => Some(decode(b)),
        w => panic!("field {f} is {w:?}"),
    }
}

fn text(m: &[(u32, Wire)], f: u32) -> Option<String> {
    match field(m, f)? {
        Wire::Bytes(b) => Some(String::from_utf8(b.clone()).unwrap()),
        w => panic!("field {f} is {w:?}"),
    }
}

fn uint(m: &[(u32, Wire)], f: u32) -> Option<u64> {
    match field(m, f)? {
        Wire::Varint(v) => Some(*v),
        w => panic!("field {f} is {w:?}"),
    }
}

#[test]
fn perfetto_is_tracks_and_balanced_slices() {
    let t = sample();
    let m = Model::of(&t);
    let out = perfetto(&t);
    let packets: Vec<Vec<(u32, Wire)>> = decode(&out)
        .into_iter()
        .map(|(f, w)| {
            assert_eq!(f, 1, "a Trace is packets");
            match w {
                Wire::Bytes(b) => decode(&b),
                w => panic!("{w:?}"),
            }
        })
        .collect();
    assert_eq!(
        uint(&packets[0], 13),
        Some(1),
        "the first clears incremental state"
    );
    assert!(
        packets.iter().all(|p| uint(p, 10) == Some(1)),
        "one sequence"
    );

    // The descriptors: the process, a track a lane, a counter a series.
    let mut track_names: HashMap<u64, String> = HashMap::new();
    let mut counters: Vec<String> = Vec::new();
    let mut process = None;
    for p in &packets {
        let Some(d) = sub(p, 60) else { continue };
        let uuid = uint(&d, 1).unwrap();
        if let Some(pd) = sub(&d, 3) {
            process = Some((uint(&pd, 1), text(&pd, 6)));
        } else if let Some(td) = sub(&d, 4) {
            assert_eq!(uint(&td, 1), Some(1));
            track_names.insert(uuid, text(&td, 5).unwrap());
        } else if field(&d, 8).is_some() {
            counters.push(text(&d, 2).unwrap());
            track_names.insert(uuid, text(&d, 2).unwrap());
        } else {
            assert_eq!(uint(&d, 5), Some(1), "under the process");
            track_names.insert(uuid, text(&d, 2).unwrap());
        }
    }
    assert_eq!(process, Some((Some(1), Some("demo \"app\"".into()))));
    let mut names: Vec<&str> = track_names.values().map(String::as_str).collect();
    names.sort();
    assert_eq!(
        names,
        ["frames", "laid", "loop", "main", "memory", "parser"]
    );
    assert_eq!(counters, ["laid", "memory"]);

    // The events: in time order, each track's slices balanced.
    let events: Vec<(u64, Vec<(u32, Wire)>)> = packets
        .iter()
        .filter_map(|p| {
            let ev = sub(p, 11)?;
            Some((uint(p, 8).expect("an event has a time"), ev))
        })
        .collect();
    let descriptors = packets.iter().filter(|p| field(p, 60).is_some()).count();
    assert_eq!(descriptors, 1 + m.lanes.len() + m.series.len());
    assert_eq!(packets.len(), descriptors + events.len());
    let slices = m.slices.len();
    let points: usize = m.series.iter().map(|s| s.points.len()).sum();
    assert_eq!(events.len(), slices * 2 + m.instants.len() + points);
    assert!(events.windows(2).all(|w| w[0].0 <= w[1].0), "in time order");
    let mut open: HashMap<u64, Vec<String>> = HashMap::new();
    let mut begun = Vec::new();
    let mut note = None;
    let mut laid = Vec::new();
    for (ts, ev) in &events {
        let track = uint(ev, 11).unwrap();
        assert!(
            track_names.contains_key(&track),
            "an event on a described track"
        );
        match uint(ev, 9).unwrap() {
            1 => {
                let name = text(ev, 23).unwrap();
                assert!(text(ev, 22).is_some(), "a category");
                begun.push((track_names[&track].clone(), name.clone(), *ts));
                open.entry(track).or_default().push(name);
            }
            2 => {
                assert!(
                    open.entry(track).or_default().pop().is_some(),
                    "an end with no begin"
                );
            }
            3 => {
                if text(ev, 23).as_deref() == Some("note") {
                    let a = sub(ev, 4).unwrap();
                    assert_eq!(text(&a, 10).as_deref(), Some("text"));
                    note = text(&a, 6);
                }
            }
            4 => {
                if track_names[&track] == "laid" {
                    laid.push(uint(ev, 30).unwrap() as i64);
                } else {
                    let Some(Wire::Fixed64(bits)) = field(ev, 44) else {
                        panic!("a gauge is a double")
                    };
                    assert_eq!(f64::from_bits(*bits), 1.5e6);
                }
            }
            k => panic!("type {k}"),
        }
    }
    assert!(
        open.values().all(Vec::is_empty),
        "every begin ended: {open:?}"
    );
    assert!(begun.contains(&("frames".into(), "frame 1".into(), 0)));
    assert!(
        begun.contains(&("main".into(), "build".into(), 10_000)),
        "ns"
    );
    assert!(begun.contains(&("parser".into(), "parse".into(), 200_000)));
    assert!(begun.contains(&("loop".into(), "asleep".into(), 10_000_000)));
    assert_eq!(note.as_deref(), Some(NOTE));
    // A step over each frame it moved in: its value, then 0 at the end.
    assert_eq!(laid, [40, 0, -3, 0]);
}

// --- speedscope -----------------------------------------------------------

#[test]
fn speedscope_profiles_open_and_close_in_order() {
    let t = sample();
    let out = speedscope(&t);
    let j = parse_json(&out).unwrap_or_else(|e| panic!("{e}\n{out}"));
    assert_eq!(
        j.get("$schema").str(),
        "https://www.speedscope.app/file-format-schema.json"
    );
    assert_eq!(j.get("name").str(), "demo \"app\"");
    let frames = j.get("shared").get("frames").arr();
    let frame_names: Vec<&str> = frames.iter().map(|f| f.get("name").str()).collect();
    for n in ["frame", "build", "rows", "parse", "render", "asleep"] {
        assert!(frame_names.contains(&n), "{n} in {frame_names:?}");
    }
    let profiles = j.get("profiles").arr();
    let names: Vec<&str> = profiles.iter().map(|p| p.get("name").str()).collect();
    assert_eq!(names, ["frames", "main", "loop", "parser"]);
    let active = j.get("activeProfileIndex").num() as usize;
    assert_eq!(names[active], "main");
    for p in profiles {
        assert_eq!(p.get("type").str(), "evented");
        assert_eq!(p.get("unit").str(), "microseconds");
        let (start, end) = (p.get("startValue").num(), p.get("endValue").num());
        let mut stack: Vec<usize> = Vec::new();
        let mut last = start;
        for e in p.get("events").arr() {
            let at = e.get("at").num();
            assert!(
                at >= last && at <= end,
                "{} in order, in range",
                p.get("name").str()
            );
            last = at;
            let f = e.get("frame").num() as usize;
            assert!(f < frames.len());
            match e.get("type").str() {
                "O" => stack.push(f),
                "C" => assert_eq!(stack.pop(), Some(f), "a close matches its open"),
                k => panic!("event {k}"),
            }
        }
        assert!(stack.is_empty());
    }
}

// --- Firefox --------------------------------------------------------------

#[test]
fn firefox_is_a_gecko_profile_of_markers() {
    let t = sample();
    let out = firefox(&t);
    let j = parse_json(&out).unwrap_or_else(|e| panic!("{e}\n{out}"));
    for k in ["meta", "libs", "threads", "pausedRanges", "processes"] {
        assert!(j.has(k), "{k}");
    }
    let meta = j.get("meta");
    assert_eq!(meta.get("version").num(), firefox::VERSION as f64);
    assert!(!meta.has("preprocessedProfileVersion"), "a Gecko profile");
    for k in [
        "startTime",
        "shutdownTime",
        "interval",
        "stackwalk",
        "debug",
        "gcpoison",
        "asyncstack",
        "processType",
    ] {
        assert!(meta.has(k), "meta.{k}");
    }
    let cats = meta.get("categories").arr();
    assert!(
        cats.iter().any(|c| c.get("color").str() == "grey"),
        "a grey category"
    );
    assert!(
        cats.iter()
            .all(|c| !c.get("subcategories").arr().is_empty())
    );
    let schema = &meta.get("markerSchema").arr()[0];
    assert_eq!(schema.get("name").str(), "kui");
    let keys: Vec<&str> = schema
        .get("data")
        .arr()
        .iter()
        .map(|f| f.get("key").str())
        .collect();
    let threads = j.get("threads").arr();
    let names: Vec<&str> = threads.iter().map(|t| t.get("name").str()).collect();
    assert_eq!(names, ["frames", "main", "loop", "parser"]);
    let mut seen: Vec<String> = Vec::new();
    let mut note = None;
    for th in threads {
        for k in [
            "processType",
            "registerTime",
            "unregisterTime",
            "tid",
            "pid",
            "samples",
            "frameTable",
            "stackTable",
        ] {
            assert!(th.has(k), "thread.{k}");
        }
        let ms = th.get("markers");
        let sc = ms.get("schema");
        assert_eq!(
            [
                sc.get("name").num(),
                sc.get("startTime").num(),
                sc.get("endTime").num(),
                sc.get("phase").num(),
                sc.get("category").num(),
                sc.get("data").num()
            ],
            [0.0, 1.0, 2.0, 3.0, 4.0, 5.0]
        );
        let strings: Vec<&str> = th.get("stringTable").arr().iter().map(Json::str).collect();
        for m in ms.get("data").arr() {
            let m = m.arr();
            assert_eq!(m.len(), 6);
            let name = strings[m[0].num() as usize];
            let phase = m[3].num();
            if phase == 1.0 {
                assert!(m[2].num() >= m[1].num(), "{name} ends after it starts");
            } else {
                assert_eq!(phase, 0.0);
                assert_eq!(m[2], Json::Null);
            }
            assert!((m[4].num() as usize) < cats.len());
            let data = &m[5];
            assert_eq!(data.get("type").str(), "kui");
            if let Json::Obj(kv) = data {
                for (k, _) in kv.iter().skip(1) {
                    assert!(keys.contains(&k.as_str()), "{k} in the schema");
                }
            }
            if name == "note" {
                note = Some(data.get("text").str().to_string());
            }
            seen.push(format!("{} {name}", th.get("name").str()));
        }
    }
    for s in [
        "frames frame",
        "main build",
        "main rows",
        "parser parse",
        "parser chunk",
        "loop asleep",
        "frames laid",
        "main input key",
    ] {
        assert!(seen.iter().any(|x| x == s), "{s} in {seen:?}");
    }
    assert_eq!(note.as_deref(), Some(NOTE));
}

// --- every adapter, an empty recording ------------------------------------

#[test]
fn an_empty_recording_writes_a_valid_file_in_each_format() {
    let t = Trace::new();
    parse_json(&chrome(&t)).unwrap();
    let s = speedscope(&t);
    let j = parse_json(&s).unwrap();
    assert_eq!(j.get("profiles").arr().len(), 1);
    let f = parse_json(&firefox(&t)).unwrap();
    assert!(
        !f.get("threads").arr().is_empty(),
        "the profiler needs a thread"
    );
    let p = decode(&perfetto(&t));
    assert_eq!(p.len(), 3, "the process, frames and main");
    assert_eq!(write(&t, Format::Kuitrace), t.to_bytes());
}

#[test]
fn formats_read_from_paths_and_names() {
    assert_eq!(Format::from_path("a.SPEEDSCOPE.json"), Format::Speedscope);
    assert_eq!(Format::from_path("a.firefox.json"), Format::Firefox);
    assert_eq!(Format::from_path("a.json"), Format::Chrome);
    assert_eq!(Format::from_path("a.perfetto-trace"), Format::Perfetto);
    assert_eq!(Format::from_path("a.kuitrace"), Format::Kuitrace);
    for f in [
        Format::Kuitrace,
        Format::Chrome,
        Format::Perfetto,
        Format::Speedscope,
        Format::Firefox,
    ] {
        assert_eq!(Format::parse(f.name()), Some(f));
    }
}
