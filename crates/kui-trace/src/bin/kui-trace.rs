//! `kui-trace`: read, convert and compare kui recordings (ADR 0058).
//!
//! ```text
//! kui-trace dump IN
//! kui-trace stats IN
//! kui-trace convert IN [--to FORMAT] OUT
//! kui-trace diff A B [--fail-over N%]
//! ```

use std::fmt::Write as _;
use std::process::ExitCode;

use kui_trace::adapters::{self, Format};
use kui_trace::{Trace, analysis, diff};

const USAGE: &str = "\
usage:
  kui-trace dump IN                         every record, a line each
  kui-trace stats IN                        frames, work, burns, the top spans, gaps
  kui-trace convert IN [--to FORMAT] OUT    write IN in another format
  kui-trace diff A B [--fail-over N%]       compare two recordings

IN, A and B are .kuitrace files.
FORMAT is chrome, perfetto, speedscope, firefox or kuitrace; without --to
it is OUT's extension: .json chrome, .pftrace / .perfetto-trace perfetto,
.speedscope.json speedscope, .firefox.json firefox, anything else kuitrace.
--fail-over exits 1 when a frame's cost grew past N percent (`10%`, `10`
or `0.1x`), the table printed either way.";

/// A command line read.
#[derive(Clone, Debug, PartialEq)]
enum Cmd {
    Help,
    Dump(String),
    Stats(String),
    Convert {
        input: String,
        output: String,
        format: Format,
    },
    Diff {
        a: String,
        b: String,
        /// A fraction: `10%` is `0.1`.
        fail_over: Option<f64>,
    },
}

/// A threshold as `--fail-over` takes it: `10%` or `10` (percent), or
/// `0.1x` (a fraction).
fn threshold(s: &str) -> Result<f64, String> {
    let bad = || format!("--fail-over: not a percentage: {s}");
    let v = if let Some(p) = s.strip_suffix('%') {
        p.trim().parse::<f64>().map_err(|_| bad())? / 100.0
    } else if let Some(x) = s.strip_suffix('x') {
        x.trim().parse::<f64>().map_err(|_| bad())?
    } else {
        s.trim().parse::<f64>().map_err(|_| bad())? / 100.0
    };
    if v.is_finite() && v >= 0.0 {
        Ok(v)
    } else {
        Err(bad())
    }
}

/// The command `args` (without the program's name) say.
fn parse(args: &[String]) -> Result<Cmd, String> {
    let mut rest: Vec<&str> = Vec::new();
    let mut to: Option<&str> = None;
    let mut fail: Option<&str> = None;
    let mut it = args.iter().map(String::as_str);
    while let Some(a) = it.next() {
        match a {
            "-h" | "--help" | "help" => return Ok(Cmd::Help),
            "--to" => to = Some(it.next().ok_or("--to wants a format")?),
            "--fail-over" => fail = Some(it.next().ok_or("--fail-over wants a percentage")?),
            a if a.starts_with("--to=") => to = Some(&a[5..]),
            a if a.starts_with("--fail-over=") => fail = Some(&a[12..]),
            a if a.starts_with("--") => return Err(format!("unknown option {a}")),
            a => rest.push(a),
        }
    }
    let (cmd, operands) = match rest.split_first() {
        Some((c, o)) => (*c, o),
        None => return Ok(Cmd::Help),
    };
    if to.is_some() && cmd != "convert" {
        return Err("--to is for convert".into());
    }
    if fail.is_some() && cmd != "diff" {
        return Err("--fail-over is for diff".into());
    }
    let s = |i: usize| operands[i].to_string();
    match (cmd, operands.len()) {
        ("dump", 1) => Ok(Cmd::Dump(s(0))),
        ("stats", 1) => Ok(Cmd::Stats(s(0))),
        ("convert", 2) => {
            let format = match to {
                Some(f) => Format::parse(f).ok_or_else(|| format!("unknown format {f}"))?,
                None => Format::from_path(operands[1]),
            };
            Ok(Cmd::Convert {
                input: s(0),
                output: s(1),
                format,
            })
        }
        ("diff", 2) => Ok(Cmd::Diff {
            a: s(0),
            b: s(1),
            fail_over: fail.map(threshold).transpose()?,
        }),
        ("dump" | "stats" | "convert" | "diff", n) => {
            Err(format!("{cmd}: wrong number of operands ({n})"))
        }
        _ => Err(format!("unknown command {cmd}")),
    }
}

fn read(path: &str) -> Result<Trace, String> {
    let b = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    Trace::from_bytes(&b).map_err(|e| format!("{path}: {e}"))
}

fn ms(us: f64) -> String {
    format!("{:.3} ms", us / 1000.0)
}

/// A recording's summary: frames and seconds, work's spread, burns, the
/// ten spans with the most total, the gaps.
fn stats(t: &Trace) -> String {
    let tl = t.timeline();
    let mut out = String::new();
    if let Some(s) = &t.session {
        let _ = writeln!(
            out,
            "{} · kui {} · {} · {:.0} Hz ({:.2} ms budget)",
            s.app, s.kui, s.platform, s.refresh_hz, s.budget_ms
        );
    }
    let secs = tl.end.saturating_sub(tl.start) as f64 / 1e6;
    let n = tl.frames.len();
    let _ = writeln!(
        out,
        "{n} frames in {secs:.3} s ({:.1} a second)",
        if secs > 0.0 { n as f64 / secs } else { 0.0 }
    );
    let work: Vec<u64> = tl.frames.iter().map(|f| f.work_us()).collect();
    let (avg, p95, max) = analysis::spread(&work);
    let _ = writeln!(
        out,
        "work: avg {} · p95 {} · max {}",
        ms(avg),
        ms(p95 as f64),
        ms(max as f64)
    );
    let unchanged = tl
        .frames
        .iter()
        .filter(|f| f.unchanged == Some(true))
        .count();
    if unchanged > 0 {
        let _ = writeln!(out, "unchanged: {unchanged} of {n}");
    }
    let burns = analysis::burns(&tl, t);
    let _ = writeln!(
        out,
        "burns: {} ({} frames)",
        burns.len(),
        burns.iter().map(|b| b.len()).sum::<usize>()
    );
    let g = analysis::gaps(&tl, t);
    let _ = write!(
        out,
        "gaps: {} · handlers {} · asleep {}",
        ms(g.total as f64),
        ms(g.handlers as f64),
        ms(g.asleep as f64)
    );
    if tl.has_loop() {
        let _ = write!(out, " · busy {}", ms(g.busy as f64));
    }
    out.push('\n');
    let spans = analysis::by_span(&tl, &tl.spans);
    if !spans.is_empty() {
        let _ = writeln!(out, "\ntop spans by total:");
        let rows: Vec<[String; 6]> = spans
            .iter()
            .take(10)
            .map(|s| {
                [
                    t.name(s.name).to_string(),
                    adapters::origin_label(t, s.origin),
                    s.calls.to_string(),
                    ms(s.total as f64),
                    ms(s.self_total as f64),
                    ms(s.worst as f64),
                ]
            })
            .collect();
        let head = ["span", "origin", "calls", "total", "self", "worst"].map(String::from);
        let w: Vec<usize> = (0..6)
            .map(|k| {
                rows.iter()
                    .chain(std::iter::once(&head))
                    .map(|r| r[k].chars().count())
                    .max()
                    .unwrap_or(0)
            })
            .collect();
        for r in std::iter::once(&head).chain(rows.iter()) {
            let _ = writeln!(
                out,
                "  {:<w0$}  {:<w1$}  {:>w2$}  {:>w3$}  {:>w4$}  {:>w5$}",
                r[0],
                r[1],
                r[2],
                r[3],
                r[4],
                r[5],
                w0 = w[0],
                w1 = w[1],
                w2 = w[2],
                w3 = w[3],
                w4 = w[4],
                w5 = w[5]
            );
        }
    }
    let counters = analysis::counters(&tl);
    if !counters.is_empty() {
        let _ = writeln!(out, "\ncounters (avg, max a frame):");
        for (name, avg, max) in counters {
            let _ = writeln!(out, "  {} {avg:.1} {max}", t.name(name));
        }
    }
    out
}

fn run(cmd: Cmd) -> Result<ExitCode, String> {
    match cmd {
        Cmd::Help => {
            println!("{USAGE}");
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Dump(p) => {
            print!("{}", kui_trace::dump::dump(&read(&p)?));
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Stats(p) => {
            print!("{}", stats(&read(&p)?));
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Convert {
            input,
            output,
            format,
        } => {
            let t = read(&input)?;
            std::fs::write(&output, adapters::write(&t, format))
                .map_err(|e| format!("{output}: {e}"))?;
            eprintln!("kui-trace: wrote {output} ({})", format.name());
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Diff { a, b, fail_over } => {
            let r = diff::diff(&read(&a)?, &read(&b)?);
            let mark = fail_over.unwrap_or(diff::MARK);
            print!("{}", r.text_with(mark));
            let worst = r.worst_growth();
            println!("\nworst growth: {:+.1}%", worst * 100.0);
            match fail_over {
                Some(limit) if worst > limit => {
                    eprintln!(
                        "kui-trace: {:+.1}% is over the {:.1}% allowed",
                        worst * 100.0,
                        limit * 100.0
                    );
                    Ok(ExitCode::FAILURE)
                }
                _ => Ok(ExitCode::SUCCESS),
            }
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = match parse(&args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("kui-trace: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let help = cmd == Cmd::Help && args.is_empty();
    match run(cmd) {
        // No arguments at all is a usage error, after the usage.
        Ok(_) if help => ExitCode::from(2),
        Ok(code) => code,
        Err(e) => {
            eprintln!("kui-trace: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Result<Cmd, String> {
        let args: Vec<String> = s.split_whitespace().map(String::from).collect();
        parse(&args)
    }

    #[test]
    fn commands_read() {
        assert_eq!(p("dump a.kuitrace"), Ok(Cmd::Dump("a.kuitrace".into())));
        assert_eq!(p("stats a.kuitrace"), Ok(Cmd::Stats("a.kuitrace".into())));
        assert_eq!(p(""), Ok(Cmd::Help));
        assert_eq!(p("--help"), Ok(Cmd::Help));
        assert!(p("dump").is_err());
        assert!(p("dump a b").is_err());
        assert!(p("frob a").is_err());
        assert!(p("dump a --bogus").is_err());
    }

    #[test]
    fn convert_takes_to_or_the_extension() {
        let c = |format| Cmd::Convert {
            input: "a.kuitrace".into(),
            output: "out.json".into(),
            format,
        };
        assert_eq!(p("convert a.kuitrace out.json"), Ok(c(Format::Chrome)));
        assert_eq!(
            p("convert a.kuitrace --to perfetto out.json"),
            Ok(c(Format::Perfetto))
        );
        assert_eq!(
            p("convert --to=speedscope a.kuitrace out.json"),
            Ok(c(Format::Speedscope))
        );
        assert!(p("convert a.kuitrace --to nope out.json").is_err());
        assert!(p("convert a.kuitrace --to").is_err());
        assert!(p("dump a.kuitrace --to chrome").is_err());
        match p("convert a.kuitrace b.firefox.json").unwrap() {
            Cmd::Convert { format, .. } => assert_eq!(format, Format::Firefox),
            c => panic!("{c:?}"),
        }
        match p("convert a.kuitrace b.pftrace").unwrap() {
            Cmd::Convert { format, .. } => assert_eq!(format, Format::Perfetto),
            c => panic!("{c:?}"),
        }
    }

    #[test]
    fn diff_takes_a_threshold() {
        let d = |f| Cmd::Diff {
            a: "a".into(),
            b: "b".into(),
            fail_over: f,
        };
        assert_eq!(p("diff a b"), Ok(d(None)));
        assert_eq!(p("diff a b --fail-over 10%"), Ok(d(Some(0.1))));
        assert_eq!(p("diff --fail-over=25 a b"), Ok(d(Some(0.25))));
        assert_eq!(p("diff a b --fail-over 0.5x"), Ok(d(Some(0.5))));
        assert!(p("diff a b --fail-over lots").is_err());
        assert!(p("diff a b --fail-over -5%").is_err());
        assert!(p("diff a").is_err());
        assert!(p("stats a --fail-over 10%").is_err());
    }
}
