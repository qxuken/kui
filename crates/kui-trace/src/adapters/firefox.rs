//! The Firefox Profiler's, as a Gecko profile of markers.
//!
//! The profiler reads two JSON shapes: its processed profile, and the
//! Gecko profile Firefox writes, which it upgrades and processes on
//! import. The processed one changes shape often (its version passed 75,
//! with tables moved, typed and renamed along the way), so a writer
//! outside the profiler's tree would chase it; the Gecko one has kept its
//! shape for years, and its upgraders from version 31 on only add a
//! sources table and fill marker schema labels. So this writes a Gecko
//! profile at version 31 (`src/types/gecko-profile.ts`), which every
//! profiler since then imports.
//!
//! A thread a lane (pid 1, tid as Chrome's), each with no samples and its
//! own string table; a slice is an interval marker, an instant an instant
//! marker, a series' point an instant marker on `frames` named for it.
//! Every marker's payload is a `kui` marker with its arguments as fields,
//! described once in the meta's marker schema; its category is its
//! slice's (a span's origin). Times are ms after the recording began, and
//! the profile's start the wall clock there.

use std::fmt::Write;

use super::json::{ms, number, string, val};
use super::{Model, Val};

/// The Gecko profile version written.
pub const VERSION: u32 = 31;

const CATEGORIES: [(&str, &str); 10] = [
    ("Other", "grey"),
    ("Frame", "purple"),
    ("kui", "blue"),
    ("Host", "green"),
    ("Extension", "orange"),
    ("Input", "yellow"),
    ("Loop", "lightblue"),
    ("Mark", "magenta"),
    ("Present", "brown"),
    ("Counter", "darkgrey"),
];

fn category(cat: &str) -> usize {
    match cat {
        "frame" => 1,
        "kui" => 2,
        "host" => 3,
        "input" | "event" | "wake" | "request" => 5,
        "loop" => 6,
        "mark" | "note" => 7,
        "present" => 8,
        "counter" | "gauge" => 9,
        // An extension's namespace.
        _ => 4,
    }
}

fn label(key: &str) -> &str {
    match key {
        "origin" => "Origin",
        "node" => "Node",
        "cpu_us" => "CPU",
        "self_us" => "Self",
        "id" => "Span",
        "parent" => "Parent span",
        "cut" => "Cut at the frame's end",
        "frame" => "Frame",
        "causes" => "Causes",
        "holders" => "Holders",
        "unchanged" => "Unchanged",
        "nodes" => "Nodes",
        "render_us" => "Render",
        "wait_us" => "Vsync wait",
        "unfinished" => "Unfinished",
        "until_us" => "Until",
        "woke_by" => "Woke by",
        "kind" => "Kind",
        "count" => "Count",
        "os_us" => "OS time",
        "at" => "Asked at",
        "text" => "Text",
        "value" => "Value",
        k => k,
    }
}

/// The marker schema's field format for an argument.
fn format(key: &str, v: &Val) -> &'static str {
    if key.ends_with("_us") {
        return "microseconds";
    }
    match v {
        Val::U(_) | Val::I(_) => "integer",
        Val::F(_) => "decimal",
        Val::Str(_) | Val::B(_) => "string",
    }
}

/// A thread's string table.
#[derive(Default)]
struct Strings(Vec<String>);

impl Strings {
    fn get(&mut self, s: &str) -> usize {
        match self.0.iter().position(|x| x == s) {
            Some(i) => i,
            None => {
                self.0.push(s.to_string());
                self.0.len() - 1
            }
        }
    }
}

fn payload(out: &mut String, args: &[(&str, Val)]) {
    out.push_str("{\"type\":\"kui\"");
    for (k, v) in args {
        out.push(',');
        string(out, k);
        out.push(':');
        match v {
            // A boolean in a string field, as the schema says.
            Val::B(b) => string(out, if *b { "yes" } else { "no" }),
            v => val(out, v),
        }
    }
    out.push('}');
}

pub fn write(m: &Model) -> String {
    // The fields any marker carries, first seen first.
    let mut fields: Vec<(&str, &'static str)> = Vec::new();
    let mut see = |k: &'static str, v: &Val| {
        if !fields.iter().any(|f| f.0 == k) {
            fields.push((k, format(k, v)));
        }
    };
    for s in &m.slices {
        for (k, v) in &s.args {
            see(k, v);
        }
    }
    for i in &m.instants {
        for (k, v) in &i.args {
            see(k, v);
        }
    }
    if !m.series.is_empty() {
        see("value", &Val::F(0.0));
    }

    let mut out = String::from("{\"meta\":{");
    let _ = write!(out, "\"version\":{VERSION},\"startTime\":");
    number(&mut out, m.wall_ms as f64);
    out.push_str(",\"shutdownTime\":null,\"categories\":[");
    for (i, (name, color)) in CATEGORIES.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"name\":");
        string(&mut out, name);
        out.push_str(",\"color\":");
        string(&mut out, color);
        out.push_str(",\"subcategories\":[\"Other\"]}");
    }
    out.push_str("],\"markerSchema\":[{\"name\":\"kui\",\"display\":[\"marker-chart\",\"marker-table\",\"timeline-overview\"],\"data\":[");
    for (i, (k, fmt)) in fields.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"key\":");
        string(&mut out, k);
        out.push_str(",\"label\":");
        string(&mut out, label(k));
        let _ = write!(out, ",\"format\":\"{fmt}\"");
        if *fmt == "string" {
            out.push_str(",\"searchable\":true");
        }
        out.push('}');
    }
    out.push_str("]}],\"interval\":1,\"stackwalk\":0,\"debug\":0,\"gcpoison\":0,\"asyncstack\":0,\"processType\":0,\"product\":");
    string(&mut out, &m.app);
    out.push_str(",\"misc\":\"kui-trace\",\"presymbolicated\":true,\"profilingStartTime\":");
    ms(&mut out, m.start);
    out.push_str(",\"profilingEndTime\":");
    ms(&mut out, m.end.max(m.start));
    out.push_str("},\"libs\":[],\"pages\":[],\"pausedRanges\":[],\"processes\":[],\"threads\":[");

    for (li, lane) in m.lanes.iter().enumerate() {
        if li > 0 {
            out.push(',');
        }
        let mut strings = Strings::default();
        let mut markers: Vec<String> = Vec::new();
        for s in m.slices.iter().filter(|s| s.lane == li) {
            let mut mk = String::new();
            let _ = write!(mk, "[{},", strings.get(&s.name));
            ms(&mut mk, s.start);
            mk.push(',');
            ms(&mut mk, s.end);
            let _ = write!(mk, ",1,{},", category(&s.cat));
            payload(&mut mk, &s.args);
            mk.push(']');
            markers.push(mk);
        }
        for i in m.instants.iter().filter(|i| i.lane == li) {
            let mut mk = String::new();
            let _ = write!(mk, "[{},", strings.get(&i.name));
            ms(&mut mk, i.t);
            let _ = write!(mk, ",null,0,{},", category(i.cat));
            payload(&mut mk, &i.args);
            mk.push(']');
            markers.push(mk);
        }
        if li == super::FRAMES {
            for s in &m.series {
                let cat = if s.gauge { "gauge" } else { "counter" };
                for (t, v) in &s.points {
                    let mut mk = String::new();
                    let _ = write!(mk, "[{},", strings.get(&s.name));
                    ms(&mut mk, *t);
                    let _ = write!(mk, ",null,0,{},", category(cat));
                    payload(&mut mk, &[("value", v.clone())]);
                    mk.push(']');
                    markers.push(mk);
                }
            }
        }

        out.push_str("{\"name\":");
        string(&mut out, &lane.name);
        out.push_str(",\"processType\":\"default\",\"processName\":");
        string(&mut out, &m.app);
        let _ = write!(
            out,
            ",\"registerTime\":0,\"unregisterTime\":null,\"tid\":{},\"pid\":1,",
            lane.tid
        );
        out.push_str("\"markers\":{\"schema\":{\"name\":0,\"startTime\":1,\"endTime\":2,\"phase\":3,\"category\":4,\"data\":5},\"data\":[");
        out.push_str(&markers.join(","));
        out.push_str(
            "]},\"samples\":{\"schema\":{\"stack\":0,\"time\":1,\"eventDelay\":2},\"data\":[]},",
        );
        out.push_str("\"frameTable\":{\"schema\":{\"location\":0,\"relevantForJS\":1,\"innerWindowID\":2,\"implementation\":3,\"line\":4,\"column\":5,\"category\":6,\"subcategory\":7},\"data\":[]},");
        out.push_str(
            "\"stackTable\":{\"schema\":{\"prefix\":0,\"frame\":1},\"data\":[]},\"stringTable\":[",
        );
        for (i, s) in strings.0.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            string(&mut out, s);
        }
        out.push_str("]}");
    }
    out.push_str("]}\n");
    out
}
