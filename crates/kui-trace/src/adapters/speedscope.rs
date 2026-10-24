//! speedscope's file format (`https://www.speedscope.app/file-format-schema.json`):
//! the frames shared, then an evented profile a lane that has slices —
//! `O` and `C` events in order, each close matching the open before it,
//! times in µs on the recording's clock. A frame is a name and, as its
//! `file`, the slice's category (a span's origin), so `build` by kui and
//! by an app are two. Frames are all `frame`, so speedscope's left-heavy
//! and sandwich views sum them; instants and series are dropped, as
//! speedscope has neither.

use std::fmt::Write;

use super::Model;
use super::json::{number, string};

pub fn write(m: &Model) -> String {
    let mut frames: Vec<(&str, &str)> = Vec::new();
    let mut frame_of = Vec::with_capacity(m.slices.len());
    for s in &m.slices {
        let key = (s.name.as_str(), s.cat.as_str());
        let i = match frames.iter().position(|f| *f == key) {
            Some(i) => i,
            None => {
                frames.push(key);
                frames.len() - 1
            }
        };
        frame_of.push(i);
    }

    let mut out = String::from(
        "{\"$schema\":\"https://www.speedscope.app/file-format-schema.json\",\"shared\":{\"frames\":[",
    );
    for (i, (name, file)) in frames.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"name\":");
        string(&mut out, name);
        out.push_str(",\"file\":");
        string(&mut out, file);
        out.push('}');
    }
    out.push_str("]},\"profiles\":[");

    // The lanes with slices; `main` even when empty, as a file needs a
    // profile.
    let lanes: Vec<usize> = (0..m.lanes.len())
        .filter(|&l| m.has_slices(l) || l == super::MAIN)
        .collect();
    let mut active = 0;
    for (k, &lane) in lanes.iter().enumerate() {
        if lane == super::MAIN {
            active = k;
        }
        let edges = m.edges(lane);
        let start = m.start.min(edges.first().map_or(m.start, |e| e.0));
        let end = m.end.max(edges.last().map_or(m.end, |e| e.0));
        if k > 0 {
            out.push(',');
        }
        out.push_str("{\"type\":\"evented\",\"name\":");
        string(&mut out, &m.lanes[lane].name);
        out.push_str(",\"unit\":\"microseconds\",\"startValue\":");
        number(&mut out, start as f64);
        out.push_str(",\"endValue\":");
        number(&mut out, end as f64);
        out.push_str(",\"events\":[");
        for (j, (at, opens, i)) in edges.iter().enumerate() {
            if j > 0 {
                out.push(',');
            }
            let _ = write!(
                out,
                "{{\"type\":\"{}\",\"frame\":{},\"at\":{at}}}",
                if *opens { 'O' } else { 'C' },
                frame_of[*i]
            );
        }
        out.push_str("]}");
    }
    out.push_str("],\"name\":");
    string(&mut out, &m.app);
    let _ = writeln!(
        out,
        ",\"activeProfileIndex\":{active},\"exporter\":\"kui-trace {}\"}}",
        env!("CARGO_PKG_VERSION")
    );
    out
}
