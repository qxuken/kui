//! One-shot perf exploration. Parsing and terminal formatting only: all
//! measurements, scope resolution and limits belong to kui-trace (ADR 0078).

use kui_core::Value as V;
use kui_trace::query::{self, Explorer, Operation, Query, Selection};
use kui_trace::{Stream, Trace};

const HELP: &str = "\
kui perf — explore a recording in the viewer or terminal

usage:
  kui perf [RECORDING] [run options] [--frame ID | --span ID | --frames FIRST:LAST | --time FROM:TO]
  kui perf summary FILE [options]
  kui perf frames FILE [options]
  kui perf frame FILE FRAME_ID [--json]
  kui perf spans FILE [options]
  kui perf span FILE SPAN_ID [--json]
  kui perf counters FILE [options]       counters and gauges
  kui perf marks FILE [options]          named instants, including repeated names
  kui perf compare BEFORE AFTER [options]

Queries run once without creating a window. FILE can be absolute or relative.
Use ./frames to open a recording whose name is a query word.

scope (summary and lists):
  --frames FIRST:LAST      recorded frame numbers, inclusive; never filtered row positions
  --time FROM:TO           half-open time range, with units on each endpoint (1s:1500ms)
                          expands to complete intersecting frames, including preceding gaps
  --verdict KINDS          comma-separated late,blocked,slow,waited,wasted,burn,fine
  --hide-unchanged         select only frames not recorded as unchanged

span attribution (spans and compare; does not change frame denominators):
  --name TEXT             case-insensitive substring, including component fill names
  --origin ORIGINS        comma-separated kui,host,extension names; repeatable
  --min-us N              minimum inclusive span duration, in microseconds
  --hide-name NAME        exclude this exact recorded name; repeatable
  --include-devtools      devtools spans are excluded by default, as in the viewer

lists:
  --sort FIELD            frames: work (default), frame, start
                          spans: self (default), total, worst, calls, name
                          marks: start (default), name; counters: name
  --asc | --desc          direction; costs default descending, names/times/frame numbers ascending
  --limit N              return at most N rows (default 20); omitted counts are shown
  --all                  return all matches without a hidden cap

comparison:
  --before-frames FIRST:LAST | --before-time FROM:TO
  --after-frames FIRST:LAST  | --after-time FROM:TO
                          choose each input's scope independently; otherwise whole files
  --limit N | --all        metric rows, overview first (default 20)
                          absolute and per-frame costs; missing measurements stay null
                          no overall winner or automatic regression failure

output:
  --json                  structured results, schema_version 1; times are milliseconds
                          records scope, filters, units, sorting, counts and omissions
  -h, --help              this help

Frame/span details are complete, by recording-local ID, independent of filters.
Time selectors accept us, ms or s; ranges and IDs must be nonnegative.
Work is core duration plus rendering, excluding vsync wait. Without both a
core end and present measurement, work and verdict stay unavailable (null in
JSON, — in text); missing work sorts last in either direction.
Examples:
  kui perf frames run.kuitrace --sort work --limit 10 --json
  kui perf spans run.kuitrace --frames 120:180 --name layout --all
  kui perf compare before.kuitrace after.kuitrace --before-time 1s:2s --after-time 3s:4s --all
  kui perf run.kuitrace --frame 123
";

pub fn handles(args: &[String]) -> bool {
    args.first().is_some_and(|s| {
        matches!(
            s.as_str(),
            "summary"
                | "frames"
                | "frame"
                | "spans"
                | "span"
                | "counters"
                | "marks"
                | "compare"
                | "--help"
                | "-h"
                | "help"
        )
    })
}

fn id(s: &str) -> Result<u64, String> {
    s.parse()
        .map_err(|_| format!("invalid nonnegative ID: {s}"))
}
fn frames(s: &str) -> Result<Selection, String> {
    query::frame_range(s)
}
fn time(s: &str) -> Result<Selection, String> {
    query::time_range(s)
}
pub fn validate_focus(flag: &str, value: &str) -> Result<(), String> {
    match flag {
        "--frame" => {
            id(value)?;
        }
        "--span" => {
            value
                .parse::<u32>()
                .map_err(|_| "span ID must fit an unsigned 32-bit integer")?;
        }
        "--frames" => {
            frames(value)?;
        }
        "--time" => {
            time(value)?;
        }
        _ => return Err("unknown focus selector".into()),
    }
    Ok(())
}

fn select(into: &mut Selection, value: Selection) -> Result<(), String> {
    if *into != Selection::Whole {
        return Err("choose only one frame or time selection for each input".into());
    }
    *into = value;
    Ok(())
}

struct Command {
    before: String,
    after: Option<String>,
    query: Query,
    after_selection: Selection,
    json: bool,
}

fn parse(args: &[String]) -> Result<Command, String> {
    let op = args.first().ok_or("missing perf operation")?;
    let mut positional = Vec::new();
    let mut q = Query::new(match op.as_str() {
        "summary" | "compare" => Operation::Summary,
        "frames" => Operation::Frames,
        "frame" => Operation::Frame(0),
        "spans" => Operation::Spans,
        "span" => Operation::Span(0),
        "counters" => Operation::Counters,
        "marks" => Operation::Marks,
        _ => return Err(format!("unknown perf operation {op}")),
    });
    if op == "compare" {
        q.limit = Some(20);
    }
    q.filter.hide_names.push("devtools".into());
    let mut after_selection = Selection::Whole;
    let mut json = false;
    let mut sort = None;
    let mut direction = None;
    let mut limit_seen = false;
    let mut scoped = false;
    let mut attributed = false;
    let mut it = args[1..].iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--json" => json = true,
            "--all" => {
                if limit_seen {
                    return Err("choose --limit or --all once".into());
                }
                q.limit = None;
                limit_seen = true;
            }
            "--asc" => direction = Some(false),
            "--desc" => direction = Some(true),
            "--hide-unchanged" => {
                q.filter.hide_unchanged = true;
                scoped = true;
            }
            "--include-devtools" => {
                q.filter.hide_names.retain(|n| n != "devtools");
                attributed = true;
            }
            "--frames" | "--time" | "--before-frames" | "--before-time" | "--after-frames"
            | "--after-time" => {
                if op == "compare" && matches!(arg.as_str(), "--frames" | "--time") {
                    return Err(
                        "compare uses --before-frames/--after-frames or --before-time/--after-time"
                            .into(),
                    );
                }
                if op != "compare" && (arg.starts_with("--before-") || arg.starts_with("--after-"))
                {
                    return Err(format!("{arg} is only for compare"));
                }
                let v = it.next().ok_or_else(|| format!("{arg} needs a range"))?;
                let s = if arg.ends_with("frames") {
                    frames(v)?
                } else {
                    time(v)?
                };
                select(
                    if arg.starts_with("--after-") {
                        &mut after_selection
                    } else {
                        &mut q.selection
                    },
                    s,
                )?;
                scoped = true;
            }
            "--limit" => {
                if limit_seen {
                    return Err("choose --limit or --all once".into());
                }
                q.limit = Some(
                    it.next()
                        .ok_or("--limit needs a number")?
                        .parse()
                        .map_err(|_| "--limit needs a nonnegative integer")?,
                );
                limit_seen = true;
            }
            "--sort" => sort = Some(it.next().ok_or("--sort needs a field")?.clone()),
            "--name" => {
                q.filter.name = Some(it.next().ok_or("--name needs text")?.clone());
                attributed = true;
            }
            "--origin" => {
                q.filter.origins.get_or_insert_with(Vec::new).extend(
                    it.next()
                        .ok_or("--origin needs names")?
                        .split(',')
                        .map(str::to_owned),
                );
                attributed = true;
            }
            "--hide-name" => {
                q.filter
                    .hide_names
                    .push(it.next().ok_or("--hide-name needs a name")?.clone());
                attributed = true;
            }
            "--min-us" => {
                q.filter.min_us = it
                    .next()
                    .ok_or("--min-us needs microseconds")?
                    .parse()
                    .map_err(|_| "--min-us needs a nonnegative integer")?;
                attributed = true;
            }
            "--verdict" => {
                let vs: Vec<_> = it
                    .next()
                    .ok_or("--verdict needs kinds")?
                    .split(',')
                    .map(str::to_owned)
                    .collect();
                if vs.iter().any(|v| {
                    ![
                        "late", "blocked", "slow", "waited", "wasted", "burn", "fine",
                    ]
                    .contains(&v.as_str())
                }) {
                    return Err("unknown frame verdict (see kui perf --help)".into());
                }
                q.filter.verdicts.get_or_insert_with(Vec::new).extend(vs);
                scoped = true;
            }
            "--" => {
                positional.extend(it.cloned());
                break;
            }
            s if s.starts_with('-') => {
                return Err(format!("unknown perf option {s} (see kui perf --help)"));
            }
            _ => positional.push(arg.clone()),
        }
    }
    let detail = matches!(q.operation, Operation::Frame(_) | Operation::Span(_));
    if detail && (scoped || attributed || sort.is_some() || direction.is_some() || limit_seen) {
        return Err(
            "details take only FILE ID and --json; use lists for scoped or filtered readings"
                .into(),
        );
    }
    if attributed && !matches!(q.operation, Operation::Spans) && op != "compare" {
        return Err("span attribution options are for spans and compare; they do not select frames, counters or marks".into());
    }
    if op == "compare" && (sort.is_some() || direction.is_some()) {
        return Err("compare uses a fixed overview-first metric order".into());
    }
    if op == "summary" && (sort.is_some() || direction.is_some() || limit_seen) {
        return Err("summary is complete; sorting and limits are for lists".into());
    }
    let want = if detail || op == "compare" { 2 } else { 1 };
    if positional.len() != want {
        return Err(format!(
            "{op} needs {} (see kui perf --help)",
            if op == "compare" {
                "BEFORE AFTER"
            } else if detail {
                "FILE ID"
            } else {
                "FILE"
            }
        ));
    }
    if let Some(sort) = sort {
        q.set_sort(sort);
    }
    if let Some(desc) = direction {
        q.descending = desc;
    }
    if detail {
        q.operation = match q.operation {
            Operation::Frame(_) => Operation::Frame(id(&positional[1])?),
            _ => Operation::Span(positional[1].parse().map_err(|_| "invalid span ID")?),
        };
    }
    Ok(Command {
        before: positional[0].clone(),
        after: (op == "compare").then(|| positional[1].clone()),
        query: q,
        after_selection,
        json,
    })
}

struct Recording {
    trace: Trace,
    timeline: kui_trace::Timeline,
    pending: usize,
}
fn read(path: &str) -> Result<Recording, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let mut stream = Stream::new();
    stream.push(&bytes).map_err(|e| format!("{path}: {e}"))?;
    if !stream.started() {
        return Err(format!("{path}: incomplete recording header"));
    }
    Ok(Recording {
        trace: stream.trace().clone(),
        timeline: stream.timeline().clone(),
        pending: stream.pending(),
    })
}

fn diagnose(value: &mut V, r: &Recording) {
    if r.pending > 0
        && let V::Map(fields) = value
    {
        fields.push(("pending_bytes".into(), r.pending.into()));
        if let Some((_, V::List(w))) = fields.iter_mut().find(|(k, _)| k == "limitations") {
            w.push(V::str("Trailing incomplete record bytes were not decoded."));
        }
    }
}

fn evaluate(c: &Command) -> Result<V, String> {
    let before = read(&c.before)?;
    let ex = Explorer::new(&before.trace, &before.timeline);
    if let Some(path) = &c.after {
        let after = read(path)?;
        let ax = Explorer::new(&after.trace, &after.timeline);
        let mut aq = c.query.clone();
        aq.selection = c.after_selection.clone();
        let comparison = query::compare(&ex, &c.query, &ax, &aq, c.query.limit)?;
        let mut v = kui_trace::query_value::comparison(&ex, &c.before, &ax, path, &comparison);
        if let V::Map(fields) = &mut v {
            for (key, item) in fields {
                if key == "before" {
                    diagnose(item, &before);
                } else if key == "after" {
                    diagnose(item, &after);
                }
            }
        }
        Ok(v)
    } else {
        let r = ex.execute(&c.query)?;
        let mut v = kui_trace::query_value::result(&before.trace, &before.timeline, &r, &c.before);
        diagnose(&mut v, &before);
        Ok(v)
    }
}

pub fn run(args: &[String]) -> Result<i32, String> {
    if args.first().is_some_and(|s| s == "help")
        || args.iter().any(|s| matches!(s.as_str(), "--help" | "-h"))
    {
        print!("{HELP}");
        return Ok(0);
    }
    let c = parse(args)?;
    let v = evaluate(&c)?;
    if c.json {
        let mut out = String::new();
        crate::modules::formats::write_json(&mut out, &v, true, 0);
        println!("{out}");
    } else {
        print!("{}", text(&v));
    }
    Ok(0)
}

/// Readable key/value trees and aligned scalar tables. Text is a presentation
/// of the very same value as JSON; unavailable values remain an em dash.
fn text(v: &V) -> String {
    fn scalar(v: &V) -> Option<String> {
        match v {
            V::Null => Some("—".into()),
            V::Bool(b) => Some(b.to_string()),
            V::Int(n) => Some(n.to_string()),
            V::Float(n) => Some(format!("{n:.3}")),
            V::Str(s) => Some(s.replace(['\n', '\r', '\t'], " ")),
            _ => None,
        }
    }
    fn render(v: &V, out: &mut String, depth: usize) {
        let pad = "  ".repeat(depth);
        match v {
            V::Map(fields) => {
                for (k, v) in fields {
                    if let Some(s) = scalar(v) {
                        out.push_str(&format!("{pad}{k}: {s}\n"));
                    } else {
                        out.push_str(&format!("{pad}{k}:\n"));
                        render(v, out, depth + 1);
                    }
                }
            }
            V::List(rows) if rows.iter().all(|r| matches!(r, V::Map(_))) => {
                let mut cols = Vec::<String>::new();
                for row in rows {
                    if let V::Map(fs) = row {
                        for (k, v) in fs {
                            if scalar(v).is_some() && !cols.contains(k) {
                                cols.push(k.clone());
                            }
                        }
                    }
                }
                let cells: Vec<Vec<String>> = rows
                    .iter()
                    .map(|r| {
                        cols.iter()
                            .map(|k| r.get(k).and_then(scalar).unwrap_or_else(|| "—".into()))
                            .collect()
                    })
                    .collect();
                let widths: Vec<_> = cols
                    .iter()
                    .enumerate()
                    .map(|(i, k)| {
                        cells
                            .iter()
                            .map(|r| r[i].chars().count())
                            .chain([k.len()])
                            .max()
                            .unwrap_or(0)
                    })
                    .collect();
                let line = |items: &[String]| {
                    items
                        .iter()
                        .enumerate()
                        .map(|(i, s)| {
                            format!(
                                "{s}{}",
                                " ".repeat(widths[i].saturating_sub(s.chars().count()))
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("  ")
                };
                if !cols.is_empty() {
                    out.push_str(&format!("{pad}{}\n", line(&cols)));
                    for r in cells {
                        out.push_str(&format!("{pad}{}\n", line(&r)));
                    }
                }
            }
            V::List(rows) => {
                for r in rows {
                    if let Some(s) = scalar(r) {
                        out.push_str(&format!("{pad}- {s}\n"));
                    } else {
                        render(r, out, depth);
                    }
                }
            }
            _ => {
                if let Some(s) = scalar(v) {
                    out.push_str(&format!("{pad}{s}\n"));
                }
            }
        }
    }
    fn context(v: &V, out: &mut String) {
        let get = |k: &str| v.get(k).and_then(scalar).unwrap_or_else(|| "—".into());
        out.push_str(&format!("Recording: {}\n", get("recording")));
        if let Some(session) = v.get("session")
            && !matches!(session, V::Null)
        {
            out.push_str(&format!(
                "Session: {} · {} · {} Hz · kui {}\n",
                session.get_str("app").unwrap_or("—"),
                session.get_str("platform").unwrap_or("—"),
                session
                    .get("refresh_hz")
                    .and_then(scalar)
                    .unwrap_or_else(|| "—".into()),
                session.get_str("kui").unwrap_or("—")
            ));
        }
        if let Some(scope) = v.get("scope") {
            out.push_str("Requested selection:\n");
            if let Some(s) = scope.get("requested") {
                render(s, out, 1);
            }
            let value = |k: &str| scope.get(k).and_then(scalar).unwrap_or_else(|| "—".into());
            out.push_str(&format!(
                "Measured: {} frames ({}–{}), {}–{} ms; {} ms accounted across {} interval(s)\n",
                value("frame_count"),
                value("first_frame"),
                value("last_frame"),
                value("from"),
                value("to"),
                value("duration"),
                value("interval_count")
            ));
        }
        if let Some(V::Map(fs)) = v.get("filters") {
            let active = V::Map(
                fs.iter()
                    .filter(|(_, v)| {
                        !matches!(v, V::Null | V::Bool(false))
                            && !matches!(v,V::List(vs) if vs.is_empty())
                            && !matches!(v,V::Float(f) if *f==0.0)
                    })
                    .cloned()
                    .collect(),
            );
            if let V::Map(fs) = &active
                && !fs.is_empty()
            {
                out.push_str("Active filters:\n");
                render(&active, out, 1);
            }
        }
        if let Some(n) = v.get_int("pending_bytes") {
            out.push_str(&format!(
                "Data limit: {n} trailing incomplete bytes were not decoded.\n"
            ));
        }
    }
    let op = v.get_str("operation").unwrap_or("query");
    let mut out = format!("Perf {op} · times in ms · unavailable values: —\n");
    if op == "compare" {
        for name in ["before", "after"] {
            out.push_str(&format!("\n{name}:\n"));
            if let Some(value) = v.get(name) {
                context(value, &mut out);
            }
        }
    } else {
        context(v, &mut out);
    }
    if let (Some(matched), Some(returned)) = (v.get_int("matched"), v.get_int("returned"))
        && !matches!(op, "summary" | "frame" | "span")
    {
        out.push_str(&format!(
            "Showing {returned} of {matched} matches ({} omitted); sort: {}{}\n",
            matched - returned,
            v.get_str("sort").unwrap_or("overview"),
            if v.get("descending") == Some(&V::Bool(true)) {
                " descending"
            } else if op != "compare" {
                " ascending"
            } else {
                ""
            }
        ));
    }
    out.push('\n');
    if let Some(data) = v.get("data") {
        if op == "frames" {
            if let V::List(rows) = data {
                let compact = V::List(
                    rows.iter()
                        .map(|row| {
                            V::map([
                                ("frame", row.get("frame").cloned().unwrap_or_default()),
                                ("begin", row.get("begin").cloned().unwrap_or_default()),
                                ("work", row.get("work").cloned().unwrap_or_default()),
                                ("nodes", row.get("nodes").cloned().unwrap_or_default()),
                                (
                                    "verdict",
                                    row.get("verdict")
                                        .and_then(|v| v.get("kind"))
                                        .cloned()
                                        .unwrap_or_default(),
                                ),
                            ])
                        })
                        .collect(),
                );
                render(&compact, &mut out, 0);
            }
        } else if op == "summary" {
            if let V::Map(fs) = data {
                render(
                    &V::Map(
                        fs.iter()
                            .filter(|(k, _)| {
                                !matches!(k.as_str(), "work_avg" | "work_p95" | "work_max")
                            })
                            .cloned()
                            .collect(),
                    ),
                    &mut out,
                    0,
                );
            }
        } else {
            render(data, &mut out, 0);
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_owned).collect()
    }
    #[test]
    fn selectors_and_query_contract_are_checked_before_reading() {
        assert!(parse(&args("spans run --time 1s:1500ms --all --sort total --asc")).is_ok());
        for s in [
            "frames run --time 1:2",
            "frames run --time NaNms:2s",
            "frames run --frames 4:2",
            "frames run --frames 1:2 --time 1s:2s",
            "frames run --limit 1 --all",
            "compare a b --frames 1:2",
            "frame run 1 --name x",
            "summary run --limit 2",
            "marks run --wat",
        ] {
            assert!(parse(&args(s)).is_err(), "{s}");
        }
        let c = parse(&args(
            "compare a b --before-frames 10:20 --after-time 1s:2s --all",
        ))
        .unwrap();
        assert_eq!(
            c.query.selection,
            Selection::Frames {
                first: 10,
                last: 20
            }
        );
        assert_eq!(
            c.after_selection,
            Selection::Time {
                from_us: 1_000_000,
                to_us: 2_000_000
            }
        );
        assert!(c.query.limit.is_none());
    }
}
