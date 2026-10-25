//! A recording as text, one record a line (`kui-trace dump`): for a diff
//! between two runs, a grep, a bug report's body.

use std::fmt::Write;

use crate::model::Trace;
use crate::record::{LoopEvent, Origin, Record};

fn ms(us: u64) -> String {
    format!("{:>10.3}", us as f64 / 1000.0)
}

/// `trace` as text: the session, then every record in the order written,
/// its time in ms first.
pub fn dump(trace: &Trace) -> String {
    let mut out = String::new();
    let n = |id| trace.name(id);
    if let Some(s) = &trace.session {
        let _ = writeln!(
            out,
            "session kui {} · {} · {} · {:.0} Hz ({:.2} ms) · @{} · wall {}",
            s.kui, s.app, s.platform, s.refresh_hz, s.budget_ms, s.scale, s.wall_ms
        );
    }
    for (id, name) in &trace.tracks {
        let _ = writeln!(out, "track {} {}", id.0, n(*name));
    }
    for r in &trace.records {
        let line = match r {
            Record::FrameBegin { t, frame, causes } => format!(
                "{} frame+ #{frame} {}",
                ms(*t),
                causes.iter().map(|c| n(*c)).collect::<Vec<_>>().join(",")
            ),
            Record::Holder {
                frame,
                what,
                name,
                node,
            } => format!(
                "{:>10} holder #{frame} {} {}{}",
                "",
                n(*what),
                n(*name),
                if *node != 0 {
                    format!(" node {node:016x}")
                } else {
                    String::new()
                }
            ),
            Record::FrameEnd {
                t,
                frame,
                unchanged,
                nodes,
            } => format!(
                "{} frame- #{frame} {nodes} nodes{}",
                ms(*t),
                match unchanged {
                    Some(true) => " unchanged",
                    Some(false) => "",
                    None => " (untraced)",
                }
            ),
            Record::Present {
                t,
                frame,
                render,
                wait,
            } => format!(
                "{} present #{frame} render {:.3} wait {:.3}",
                ms(*t),
                *render as f64 / 1000.0,
                *wait as f64 / 1000.0
            ),
            Record::Span(s) => format!(
                "{} span {} [{}] {} {:.3} ms{}{}{}{}",
                ms(s.start),
                trace.track_name(s.track),
                match s.origin {
                    Origin::Kui => "kui".to_string(),
                    Origin::Host => "host".to_string(),
                    Origin::Extension(ns) => n(ns).to_string(),
                },
                n(s.name),
                s.dur as f64 / 1000.0,
                s.cpu
                    .map_or(String::new(), |c| format!(" cpu {:.3}", c as f64 / 1000.0)),
                if s.parent != 0 {
                    format!(" in {}", s.parent)
                } else {
                    String::new()
                },
                if s.node != 0 {
                    format!(" node {:016x}", s.node)
                } else {
                    String::new()
                },
                if s.cut { " cut" } else { "" }
            ),
            Record::Input {
                t,
                kind,
                os_t,
                count,
                until,
            } => format!(
                "{} input {}{}{}",
                ms(*t),
                n(*kind),
                if *count > 1 {
                    format!(" ×{count} until {}", ms(*until).trim())
                } else {
                    String::new()
                },
                os_t.map_or(String::new(), |o| format!(" os {}", ms(o).trim()))
            ),
            Record::Event { t, kind, node } => {
                format!("{} event {} node {node:016x}", ms(*t), n(*kind))
            }
            Record::Wake { t, name } => format!("{} wake {}", ms(*t), n(*name)),
            Record::Request { t, why, at } => {
                format!("{} request {} at {}", ms(*t), n(*why), n(*at))
            }
            Record::Loop { t, event } => match event {
                LoopEvent::Sleep { until } => format!(
                    "{} loop sleep{}",
                    ms(*t),
                    until.map_or(String::new(), |u| format!(" until {}", ms(u).trim()))
                ),
                LoopEvent::Woke { by } => format!("{} loop woke {}", ms(*t), n(*by)),
                LoopEvent::Os { kind } => format!("{} loop os {}", ms(*t), n(*kind)),
            },
            Record::Counter {
                t,
                frame,
                name,
                value,
            } => format!("{} counter #{frame} {} {value}", ms(*t), n(*name)),
            Record::Gauge { t, name, value } => {
                format!("{} gauge {} {value}", ms(*t), n(*name))
            }
            Record::Mark { t, track, name } => {
                format!("{} mark {} {}", ms(*t), trace.track_name(*track), n(*name))
            }
            Record::Note { t, text } => format!("{} note {text}", ms(*t)),
            Record::Session(_) | Record::Name { .. } | Record::Track { .. } => continue,
        };
        out.push_str(&line);
        out.push('\n');
    }
    out
}
