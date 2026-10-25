//! Chrome's trace-event JSON (the "Trace Event Format" document): an
//! object with `traceEvents`, `displayTimeUnit` and `otherData`. One
//! process (pid 1, named for the app), a thread a lane with `M` events
//! naming and ordering them; slices as complete `X` events, instants as
//! `i` (a note's scope global, the rest the thread's), counters and
//! gauges as `C` events on the process. Times are µs on the recording's
//! clock. The events are sorted by time, a parent before its children.

use std::fmt::Write;

use super::json::{object, string, val};
use super::{Model, Val};

pub fn write(m: &Model) -> String {
    let mut out = String::from("{\"traceEvents\":[\n");
    let mut first = true;
    let mut push = |out: &mut String, ev: &str| {
        if !first {
            out.push_str(",\n");
        }
        first = false;
        out.push_str(ev);
    };

    let main_tid = m.lanes[super::MAIN].tid;
    let mut ev = String::new();
    meta(
        &mut ev,
        "process_name",
        main_tid,
        &[("name", Val::Str(m.app.clone()))],
    );
    push(&mut out, &ev);
    for (i, lane) in m.lanes.iter().enumerate() {
        let mut ev = String::new();
        meta(
            &mut ev,
            "thread_name",
            lane.tid,
            &[("name", Val::Str(lane.name.clone()))],
        );
        push(&mut out, &ev);
        let mut ev = String::new();
        meta(
            &mut ev,
            "thread_sort_index",
            lane.tid,
            &[("sort_index", Val::I(i as i64))],
        );
        push(&mut out, &ev);
    }

    // Everything timed, sorted by time; the order made keeps a parent
    // before its children at one time.
    let mut timed: Vec<(u64, String)> = Vec::new();
    for s in &m.slices {
        let mut ev = String::from("{\"name\":");
        string(&mut ev, &s.label);
        ev.push_str(",\"cat\":");
        string(&mut ev, &s.cat);
        let _ = write!(
            ev,
            ",\"ph\":\"X\",\"ts\":{},\"dur\":{},\"pid\":1,\"tid\":{},\"args\":",
            s.start,
            s.end - s.start,
            m.lanes[s.lane].tid
        );
        object(&mut ev, &s.args);
        ev.push('}');
        timed.push((s.start, ev));
    }
    for i in &m.instants {
        let mut ev = String::from("{\"name\":");
        string(&mut ev, &i.name);
        ev.push_str(",\"cat\":");
        string(&mut ev, i.cat);
        let _ = write!(
            ev,
            ",\"ph\":\"i\",\"s\":\"{}\",\"ts\":{},\"pid\":1,\"tid\":{},\"args\":",
            if i.global { 'g' } else { 't' },
            i.t,
            m.lanes[i.lane].tid
        );
        object(&mut ev, &i.args);
        ev.push('}');
        timed.push((i.t, ev));
    }
    for s in &m.series {
        for (t, v) in &s.points {
            let mut ev = String::from("{\"name\":");
            string(&mut ev, &s.name);
            let _ = write!(
                ev,
                ",\"cat\":\"{}\",\"ph\":\"C\",\"ts\":{t},\"pid\":1,\"args\":{{",
                if s.gauge { "gauge" } else { "counter" }
            );
            string(&mut ev, &s.name);
            ev.push(':');
            val(&mut ev, v);
            ev.push_str("}}");
            timed.push((*t, ev));
        }
    }
    timed.sort_by_key(|e| e.0);
    for (_, ev) in &timed {
        push(&mut out, ev);
    }

    out.push_str("\n],\"displayTimeUnit\":\"ms\",\"otherData\":");
    let mut other = m.meta.clone();
    other.push(("exporter", Val::Str("kui-trace".into())));
    object(&mut out, &other);
    out.push_str("}\n");
    out
}

fn meta(ev: &mut String, name: &str, tid: u64, args: &[(&str, Val)]) {
    ev.push_str("{\"name\":");
    string(ev, name);
    let _ = write!(ev, ",\"ph\":\"M\",\"pid\":1,\"tid\":{tid},\"args\":");
    object(ev, args);
    ev.push('}');
}
