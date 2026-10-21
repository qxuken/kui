//! A kept session runs an input as a new one does (`kui_fuzz::reuse`).
//!
//! The `events` and `scenes` targets build every input's core in a
//! session kept from the inputs before it. Here every kept input of
//! theirs, and inputs made from a fixed seed, run first each on a new
//! session and then twice over on kept ones, the second time in the
//! other order, so each input runs after different ones. Every run's
//! trace must be the first run's: the events, the commands, the
//! warnings, the editor, the access tree and the quads after every step.
//! An input that panics must panic every time.
//!
//! `KUI_FUZZ_CORPUS=1` adds what `fuzz/corpus/<target>` holds, which a
//! run grows and nothing checks in.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

type Trace = fn(&[u8]) -> Vec<String>;

/// The inputs a target is checked on, and what each is called.
fn inputs(target: &str) -> Vec<(String, Vec<u8>)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut kinds = vec!["seeds", "regressions"];
    if std::env::var_os("KUI_FUZZ_CORPUS").is_some() {
        kinds.push("corpus");
    }
    let mut out = Vec::new();
    for kind in kinds {
        let Ok(entries) = std::fs::read_dir(root.join(kind).join(target)) else {
            continue;
        };
        let mut files: Vec<_> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_file() && p.extension().is_none_or(|x| x != "md"))
            .collect();
        files.sort();
        for file in files {
            let name = format!("{kind}/{}", file.file_name().unwrap().to_string_lossy());
            out.push((name, std::fs::read(&file).expect("a kept input")));
        }
    }
    // Bytes from a fixed seed: whatever `arbitrary` makes of them.
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for i in 0..48 {
        let len = 32 + (next() % 480) as usize;
        let bytes = (0..len).map(|_| next() as u8).collect();
        out.push((format!("random/{i}"), bytes));
    }
    out
}

/// The trace, or the panic's message, handles left out.
fn outcome(trace: Trace, data: &[u8]) -> Result<Vec<String>, String> {
    catch_unwind(AssertUnwindSafe(|| trace(data)))
        .map(|lines| lines.iter().map(|l| unhandled(l)).collect())
        .map_err(|e| {
            e.downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                .map(|s| unhandled(&s))
                .unwrap_or_default()
        })
}

/// `line` with every resource handle's index and generation (`SoundId(1v63)`)
/// spelled `SoundId(_)`. Handles are unique to the process, so a fixture
/// registered in a new session is not the kept session's number; window
/// and playback ids are plain counters and stay, since a kept session must
/// count them as a new one does.
fn unhandled(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(at) = rest.find("Id(") {
        let (head, tail) = rest.split_at(at + 3);
        out.push_str(head);
        let inner = tail.split(')').next().unwrap_or("");
        let handle = inner.split_once('v').is_some_and(|(i, g)| {
            !i.is_empty()
                && !g.is_empty()
                && i.bytes().all(|b| b.is_ascii_digit())
                && g.bytes().all(|b| b.is_ascii_digit())
        });
        if handle {
            out.push('_');
            rest = &tail[inner.len()..];
        } else {
            rest = tail;
        }
    }
    out.push_str(rest);
    out
}

fn first_difference(a: &Result<Vec<String>, String>, b: &Result<Vec<String>, String>) -> String {
    match (a, b) {
        (Ok(a), Ok(b)) => {
            let at = a.iter().zip(b).position(|(x, y)| x != y);
            match at {
                Some(i) => format!("line {i}:\n  new:  {:.600}\n  kept: {:.600}", a[i], b[i]),
                None => format!(
                    "{} lines on a new session, {} on a kept one",
                    a.len(),
                    b.len()
                ),
            }
        }
        (a, b) => format!(
            "new: {:.300}\nkept: {:.300}",
            format!("{a:?}"),
            format!("{b:?}")
        ),
    }
}

fn same_on_kept_sessions(target: &str, trace: Trace) {
    let inputs = inputs(target);
    assert!(inputs.len() > 48, "no kept inputs for {target}");
    kui_fuzz::reuse::set_fresh(true);
    let fresh: Vec<_> = inputs.iter().map(|(_, d)| outcome(trace, d)).collect();
    kui_fuzz::reuse::set_fresh(false);
    let before = kui_fuzz::reuse::sessions_made();
    let mut failed = Vec::new();
    for round in 0..2 {
        let mut order: Vec<usize> = (0..inputs.len()).collect();
        if round == 1 {
            order.reverse();
        }
        for i in order {
            let (name, data) = &inputs[i];
            let kept = outcome(trace, data);
            if kept != fresh[i] {
                failed.push(format!(
                    "{name} (round {round}): {}",
                    first_difference(&fresh[i], &kept)
                ));
            }
        }
    }
    let made = kui_fuzz::reuse::sessions_made() - before;
    assert!(
        failed.is_empty(),
        "{target}: {} of {} runs differ on a kept session:\n{}",
        failed.len(),
        2 * inputs.len(),
        failed.join("\n")
    );
    // Not vacuous: most runs were on a session an earlier one used.
    assert!(
        (made as usize) < inputs.len(),
        "{target}: {made} sessions made for {} runs",
        2 * inputs.len()
    );
    let panics = fresh.iter().filter(|o| o.is_err()).count();
    eprintln!(
        "{target}: {} inputs ({panics} panic), twice on kept sessions, {made} sessions made",
        inputs.len()
    );
}

#[test]
fn events_runs_the_same_on_a_kept_session() {
    same_on_kept_sessions("events", kui_fuzz::events::trace);
}

#[test]
fn scenes_runs_the_same_on_a_kept_session() {
    same_on_kept_sessions("scenes", kui_fuzz::scenes::trace);
}
