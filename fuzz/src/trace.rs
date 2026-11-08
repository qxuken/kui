//! The recording's reader (ADR 0058, ADR 0075): bytes after a recording's
//! header, read whole and as a stream in pieces the input cuts — a file
//! read while it grows, as `kui perf` and the devtools' Perf tab read one
//! — then the analyses `kui-trace stats` and kui-perf run, the shared
//! perf queries, and every adapter's output. A recording read in pieces
//! gives the same query result as one read whole. A recording written
//! back reads as the same number of records.

use kui_trace::adapters::{self, Format};
use kui_trace::query::{Explorer, Operation, Query, Selection};
use kui_trace::{Stream, Trace, analysis, header};

pub fn run(data: &[u8]) {
    let Some((&cut, body)) = data.split_first() else {
        return;
    };
    let mut bytes = header();
    bytes.extend_from_slice(body);
    let Ok(whole) = Trace::from_bytes(&bytes) else {
        return;
    };
    let step = usize::from(cut % 61) + 1;
    let mut s = Stream::new();
    for piece in bytes.chunks(step) {
        if let Err(e) = s.push(piece) {
            panic!("a stream refused what a whole read took, in pieces of {step}: {e:?}");
        }
    }
    assert_eq!(
        s.trace().records.len(),
        whole.records.len(),
        "records, in pieces of {step}"
    );
    let tl = s.timeline();
    let whole_tl = whole.timeline();
    assert_eq!(
        tl.frames.len(),
        whole_tl.frames.len(),
        "frames, in pieces of {step}"
    );
    assert_eq!(
        tl.spans.len(),
        whole_tl.spans.len(),
        "spans, in pieces of {step}"
    );
    let names = s.trace();
    check_query(
        cut,
        &Explorer::new(names, tl),
        &Explorer::new(&whole, &whole_tl),
    );
    let burns = analysis::burns(tl, names);
    for i in 0..tl.frames.len() {
        analysis::phases(tl, i, names);
        analysis::verdict(tl, i, 8_333, &burns, names);
    }
    analysis::gaps(tl, names);
    analysis::placements(tl, names);
    for f in [
        Format::Chrome,
        Format::Perfetto,
        Format::Speedscope,
        Format::Firefox,
    ] {
        adapters::write(&whole, f);
    }
    let again = adapters::write(&whole, Format::Kuitrace);
    let back = Trace::from_bytes(&again).expect("a recording written back reads");
    assert_eq!(
        back.records.len(),
        whole.records.len(),
        "records written back"
    );
}

fn check_query(cut: u8, streamed: &Explorer<'_>, whole: &Explorer<'_>) {
    // Keep the recording bytes intact while varying the query with the
    // same byte that chooses the stream's chunk size.
    let tl = whole.timeline;
    let frame = tl
        .frames
        .get(usize::from(cut >> 4) % tl.frames.len().max(1));
    let span = tl.spans.get(usize::from(cut >> 4) % tl.spans.len().max(1));
    let operation = match cut & 7 {
        0 => Operation::Summary,
        1 => Operation::Frames,
        2 => Operation::Spans,
        3 => Operation::Counters,
        4 => Operation::Marks,
        5 => Operation::Frame(frame.map_or(0, |f| f.frame)),
        6 => Operation::Span(span.map_or(0, |s| s.id)),
        _ => Operation::Spans,
    };
    let mut query = Query::new(operation);
    if let Some(frame) = frame {
        query.selection = match (cut >> 3) & 3 {
            1 => Selection::Frames {
                first: frame.frame,
                last: frame.frame,
            },
            2 => Selection::Time {
                from_us: frame.begin,
                to_us: frame.end_or_begin().max(frame.begin.saturating_add(1)),
            },
            _ => Selection::Whole,
        };
    }
    if cut & 32 != 0 {
        query.filter.hide_unchanged = true;
        query.filter.min_us = u64::from(cut);
    }
    if cut & 7 == 7 {
        query.set_sort("name");
    }
    if query.limit.is_some() {
        query.limit = match cut >> 6 {
            0 => None,
            1 => Some(0),
            2 => Some(1),
            _ => query.limit,
        };
    }
    let result = whole.execute(&query);
    assert_eq!(streamed.execute(&query), result, "{query:?}");
    if let Ok(result) = result {
        assert!(result.returned <= result.matched);
        if let Some(limit) = query.limit {
            assert_eq!(result.returned, result.matched.min(limit));
        }
    }
}
