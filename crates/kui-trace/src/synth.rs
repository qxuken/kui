//! Recordings made up, in the shape a session writes (backlog F247, ADR
//! 0075 decision 12): for a reader's tests, a bench, and the inspector's
//! spike, which needs ten minutes at 120 Hz — 72,000 frames, a million
//! spans — that no Perf tab keeps.
//!
//! Each frame's gap holds the loop asleep and woken, a run of pointer moves
//! or a key now and then, the app's handler with spans of its own, wakes
//! and asked-for frames; the frame holds `build`, the app's spans nested a
//! few deep under it, an extension's `fill` placing components — most
//! replayed, some drawn fresh for a fact that moved — then `layout` and
//! `emit`, counters, and a present. Runs of frames with no input (burns)
//! come every few hundred frames with an animation holding them; a slow
//! frame, a parse on a track of its own, gauges, marks and notes now and
//! then. Spans are written as a recorder closes them, a child before its
//! parent. The same `seed` makes the same recording.
//!
//! ```
//! let t = kui_trace::synth::Synth { frames: 120, ..Default::default() }.trace();
//! let tl = t.timeline();
//! assert_eq!(tl.frames.len(), 120);
//! assert!(tl.spans.len() >= 120 * 10);
//! ```

use std::io::{self, Write};

use crate::codec::Writer;
use crate::model::{Names, Trace};
use crate::record::{LoopEvent, NameId, Origin, Record, Session, Span, Track};

/// What to make.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Synth {
    pub frames: u64,
    /// About how many spans a frame closes, its gap's included.
    pub spans_per_frame: u32,
    pub seed: u64,
    /// The display's refresh rate: the frame budget, and how far apart
    /// frames come.
    pub hz: f32,
}

impl Default for Synth {
    fn default() -> Self {
        Synth {
            frames: 600,
            spans_per_frame: 14,
            seed: 1,
            hz: 120.0,
        }
    }
}

/// xorshift64*: enough randomness for a made-up session, the same for a
/// seed everywhere.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A fraction in `[0, 1)`.
    fn f(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// An integer in `[lo, hi]`.
    fn range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.next() % (hi - lo + 1)
    }

    fn chance(&mut self, p: f64) -> bool {
        self.f() < p
    }

    fn pick<'a>(&mut self, of: &[&'a str]) -> &'a str {
        of[self.next() as usize % of.len()]
    }
}

const APP_SPANS: &[&str] = &[
    "rows",
    "row",
    "toolbar",
    "status",
    "sidebar",
    "editor",
    "highlight",
    "diff",
    "query",
    "format",
];
const COMPONENTS: &[&str] = &[
    "kit:table",
    "kit:tabs",
    "kit:button",
    "kit:chart",
    "kit:tree",
];
const MOVED: &[&str] = &["hover", "scroll", "layout", "focused", "the clock", "theme"];
const KEYS: &[&str] = &["key", "button", "wheel", "text"];

/// Where records go: a writer, or a recording in memory.
trait Sink {
    fn put(&mut self, r: Record) -> io::Result<()>;
}

struct ToTrace<'a>(&'a mut Trace);

impl Sink for ToTrace<'_> {
    fn put(&mut self, r: Record) -> io::Result<()> {
        self.0.push(r);
        Ok(())
    }
}

impl<W: Write> Sink for Writer<W> {
    fn put(&mut self, r: Record) -> io::Result<()> {
        self.write(&r)
    }
}

/// The maker's state: names handed out, the clock, span ids.
struct Gen<'a> {
    sink: &'a mut dyn Sink,
    names: Names,
    rng: Rng,
    t: u64,
    next_id: u32,
}

impl Gen<'_> {
    fn name(&mut self, s: &str) -> io::Result<NameId> {
        let (id, new) = self.names.get(s);
        if new {
            self.sink.put(Record::Name {
                id,
                text: s.to_string(),
            })?;
        }
        Ok(id)
    }

    fn id(&mut self) -> u32 {
        self.next_id += 1;
        self.next_id
    }

    #[allow(clippy::too_many_arguments)]
    fn span(
        &mut self,
        id: u32,
        parent: u32,
        name: &str,
        origin: Origin,
        start: u64,
        dur: u64,
        node: u64,
    ) -> io::Result<()> {
        let name = self.name(name)?;
        let cpu = Some(dur - dur / 8);
        self.sink.put(Record::Span(Span {
            track: Track::MAIN,
            id,
            parent,
            name,
            origin,
            start,
            dur,
            cpu,
            node,
            cut: false,
        }))
    }

    /// A tree of the app's spans from `start` lasting `dur`, at most
    /// `budget` of them and `depth` deep, closed children first; how many
    /// it made.
    fn tree(
        &mut self,
        parent: u32,
        start: u64,
        dur: u64,
        depth: u32,
        budget: u32,
    ) -> io::Result<u32> {
        let id = self.id();
        let name = self.rng.pick(APP_SPANS);
        let node = self.rng.range(1, 400) * 0x1000_0001;
        let mut made = 1;
        if depth > 0 && budget > 1 && dur > 8 {
            let kids = self.rng.range(1, 3).min((budget - 1) as u64) as u32;
            let mut at = start + dur / 20;
            let room = dur - dur / 10;
            for k in 0..kids {
                let share = room / kids as u64;
                let len = (share as f64 * (0.4 + 0.5 * self.rng.f())) as u64;
                let left = (budget - made) / (kids - k);
                made += self.tree(id, at, len.max(1), depth - 1, left.max(1))?;
                at += share;
            }
        }
        self.span(id, parent, name, Origin::Host, start, dur, node)?;
        Ok(made)
    }
}

impl Synth {
    /// The recording, in memory.
    pub fn trace(&self) -> Trace {
        let mut t = Trace::new();
        self.run(&mut ToTrace(&mut t))
            .expect("a recording in memory cannot fail to write");
        t
    }

    /// The recording, streamed to `w` as a `.kuitrace`: one long session
    /// in a few MB of memory, whatever its length.
    pub fn write(&self, w: impl Write) -> io::Result<()> {
        let mut w = Writer::new(io::BufWriter::new(w))?;
        self.run(&mut w)?;
        w.flush()
    }

    fn run(&self, sink: &mut dyn Sink) -> io::Result<()> {
        let budget = (1e6 / self.hz.max(1.0) as f64) as u64;
        sink.put(Record::Session(Session {
            kui: "synth".into(),
            app: "synth".into(),
            platform: "synth".into(),
            refresh_hz: self.hz,
            budget_ms: budget as f32 / 1000.0,
            scale: 2.0,
            wall_ms: 1_761_000_000_000 + self.seed,
        }))?;
        let mut g = Gen {
            sink,
            names: Names::new(),
            rng: Rng(self.seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1),
            t: 0,
            next_id: 0,
        };
        let parse = g.name("parse")?;
        g.sink.put(Record::Track {
            id: Track(2),
            name: parse,
        })?;
        let per_frame = self.spans_per_frame.max(6);
        let mut burn_left = 0u64;
        for frame in 1..=self.frames {
            if burn_left == 0 && g.rng.chance(1.0 / 400.0) {
                burn_left = g.rng.range(35, 90);
            }
            let burning = burn_left > 0;
            burn_left = burn_left.saturating_sub(1);
            let mut spans = 0u32;

            // The gap: asleep, woken, and what woke it.
            let gap = if burning {
                budget
            } else {
                budget + g.rng.range(0, budget * 6)
            };
            let woke = g.t + gap - gap / 6;
            let sleep_until = Some(woke);
            g.sink.put(Record::Loop {
                t: g.t + 20,
                event: LoopEvent::Sleep { until: sleep_until },
            })?;
            let input = !burning && g.rng.chance(0.55);
            let by = if burning {
                "redraw"
            } else if input {
                "input"
            } else {
                "timer"
            };
            let by = g.name(by)?;
            g.sink.put(Record::Loop {
                t: woke,
                event: LoopEvent::Woke { by },
            })?;
            let mut causes = Vec::new();
            let mut at = woke + 10;
            if input {
                let pointer = g.rng.chance(0.7);
                let kind = if pointer {
                    "pointerMove"
                } else {
                    g.rng.pick(KEYS)
                };
                let k = g.name(kind)?;
                let count = if pointer {
                    g.rng.range(1, 12) as u32
                } else {
                    1
                };
                g.sink.put(Record::Input {
                    t: at,
                    kind: k,
                    os_t: Some(at.saturating_sub(g.rng.range(100, 900))),
                    count,
                    until: at + if count > 1 { 400 } else { 0 },
                })?;
                causes.push(k);
                at += 450;
                let ev = g.name(if pointer { "pointerMove" } else { "click" })?;
                g.sink.put(Record::Event {
                    t: at,
                    kind: ev,
                    node: g.rng.range(1, 400) * 0x1000_0001,
                })?;
                // The app's handler, a span or two inside it.
                let h = g.id();
                let len = g.rng.range(40, 600);
                let kid = g.id();
                g.span(kid, h, "update", Origin::Host, at + 5, len / 2, 0)?;
                g.span(h, 0, "handler", Origin::Host, at, len, 0)?;
                spans += 2;
                at += len + 10;
            }
            if g.rng.chance(0.08) {
                let w = g.name("parser")?;
                g.sink.put(Record::Wake { t: at, name: w })?;
                causes.push(g.name("wake: parser")?);
            }
            if burning {
                causes.push(g.name("owed")?);
            }
            if g.rng.chance(0.03) {
                let why = g.name("caret blink")?;
                let line = g.name("src/editor.rs:88")?;
                g.sink.put(Record::Request {
                    t: at,
                    why,
                    at: line,
                })?;
                causes.push(g.name("request: caret blink")?);
            }
            if g.rng.chance(0.02) {
                let what = if g.rng.chance(0.5) {
                    "file opened"
                } else {
                    "command run"
                };
                let m = g.name(what)?;
                g.sink.put(Record::Mark {
                    t: at,
                    track: Track::MAIN,
                    name: m,
                })?;
            }

            // The frame.
            let begin = woke + gap / 6 - 1;
            let begin = begin.max(at + 1);
            g.sink.put(Record::FrameBegin {
                t: begin,
                frame,
                causes,
            })?;
            if burning {
                let what = g.name("transition")?;
                let name = g.name("progress (width)")?;
                g.sink.put(Record::Holder {
                    frame,
                    what,
                    name,
                    node: 0x2000_0002,
                })?;
            }
            let slow = !burning && g.rng.chance(0.01);
            let work = if slow {
                budget * g.rng.range(2, 4)
            } else {
                (budget as f64 * (0.15 + 0.35 * g.rng.f())) as u64
            };
            let build_len = work * 6 / 10;
            let layout_len = work * 25 / 100;
            let emit_len = work.saturating_sub(build_len + layout_len + 30).max(1);
            let build = g.id();
            let b0 = begin + 10;
            // The app's spans, then an extension's fill, inside build.
            let app_room = build_len * 6 / 10;
            let app_budget = per_frame.saturating_sub(spans + 5).max(1);
            let mut made = 0;
            let mut cursor = b0 + 5;
            while made < app_budget {
                let left = app_budget - made;
                let len = (app_room / app_budget.max(1) as u64 * left.min(6) as u64).max(2);
                made += g.tree(build, cursor, len, 3, left.min(6))?;
                cursor += len + 2;
                if cursor > b0 + app_room {
                    break;
                }
            }
            spans += made;
            debug_assert!(spans <= per_frame + 8);
            let fill = g.id();
            let fill_at = b0 + app_room + 20;
            let fill_len = build_len.saturating_sub(app_room + 40).max(1);
            let kit = g.name("kit")?;
            for c in 0..g.rng.range(2, 6) {
                let kind = g.rng.pick(COMPONENTS);
                let component = g.name(kind)?;
                let fresh = g.rng.chance(if burning { 0.6 } else { 0.15 });
                let (answer, why) = if !fresh {
                    ("replayed", String::new())
                } else if g.rng.chance(0.7) {
                    let fact = g.rng.pick(MOVED);
                    let why = match fact {
                        "the clock" | "theme" => fact.to_string(),
                        f => format!("{f} {:016x}", g.rng.range(1, 400) * 0x1000_0001),
                    };
                    ("reads", why)
                } else {
                    ("params", String::new())
                };
                let answer = g.name(answer)?;
                let why = g.name(&why)?;
                g.sink.put(Record::Placement {
                    t: fill_at + 5 + c,
                    node: 0x3000_0000 + c,
                    component,
                    answer,
                    why,
                })?;
            }
            g.span(
                fill,
                build,
                "fill kit",
                Origin::Extension(kit),
                fill_at,
                fill_len,
                0,
            )?;
            if g.rng.chance(0.01) {
                g.sink.put(Record::Note {
                    t: fill_at + 2,
                    text: "editor moved, reads fs.stat".into(),
                })?;
            }
            g.span(build, 0, "build", Origin::Kui, b0, build_len, 0)?;
            let lay = g.id();
            let l0 = b0 + build_len + 5;
            g.span(lay, 0, "layout", Origin::Kui, l0, layout_len, 0)?;
            let emit = g.id();
            g.span(
                emit,
                0,
                "emit",
                Origin::Kui,
                l0 + layout_len + 5,
                emit_len,
                0,
            )?;
            let end = begin + work;
            for (name, v) in [
                ("texts shaped", g.rng.range(0, 3) as i64),
                ("glyphs rasterized", if g.rng.chance(0.05) { 26 } else { 0 }),
                ("rows", g.rng.range(30, 60) as i64),
            ] {
                if v > 0 {
                    let name = g.name(name)?;
                    g.sink.put(Record::Counter {
                        t: end,
                        frame,
                        name,
                        value: v,
                    })?;
                }
            }
            g.sink.put(Record::FrameEnd {
                t: end,
                frame,
                unchanged: Some(burning && g.rng.chance(0.5)),
                nodes: g.rng.range(300, 900) as u32,
            })?;
            let render = g.rng.range(budget / 20, budget / 6);
            let wait = budget.saturating_sub(work % budget + render);
            g.sink.put(Record::Present {
                t: end + render + wait,
                frame,
                render,
                wait,
            })?;
            if frame % 30 == 0 {
                let mem = g.name("footprint")?;
                g.sink.put(Record::Gauge {
                    t: end + 5,
                    name: mem,
                    value: 180e6 + g.rng.f() * 20e6,
                })?;
            }
            if g.rng.chance(0.01) {
                let id = g.id();
                let len = g.rng.range(budget, budget * 8);
                let start = end.saturating_sub(len);
                let name = g.name("parse")?;
                g.sink.put(Record::Span(Span {
                    track: Track(2),
                    id,
                    parent: 0,
                    name,
                    origin: Origin::Host,
                    start,
                    dur: len,
                    cpu: None,
                    node: 0,
                    cut: false,
                }))?;
            }
            g.t = end + render + wait;
        }
        Ok(())
    }
}
