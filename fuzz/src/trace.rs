//! The recording's reader (ADR 0058, ADR 0075): bytes after a recording's
//! header, read whole and as a stream in pieces the input cuts — a file
//! read while it grows, as `kui perf` and the devtools' Perf tab read one
//! — then the analyses `kui-trace stats` and kui-perf run, and every
//! adapter's output. The properties: nothing panics, a recording read in
//! pieces is the recording read whole (the same records, the same frames
//! and spans), and one written back as a recording reads as the same
//! records.

use kui_trace::adapters::{self, Format};
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
