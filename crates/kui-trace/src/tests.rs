use crate::analysis::{self, Verdict};
use crate::{LoopEvent, Origin, Record, Session, Span, Trace, Track};

/// A recording of `n` frames 16 ms apart, each with a `build` of
/// `build_us`, caused by `cause`.
fn frames(n: u64, build_us: u64, cause: &str) -> Trace {
    let mut t = Trace::new();
    let (c, _) = t.names.get(cause);
    let (build, _) = t.names.get(analysis::BUILD);
    for i in 0..n {
        let at = i * 16_000;
        t.records.push(Record::FrameBegin {
            t: at,
            frame: i + 1,
            causes: vec![c],
        });
        t.records.push(Record::Span(Span {
            track: Track::MAIN,
            id: i as u32 + 1,
            parent: 0,
            name: build,
            origin: Origin::Kui,
            start: at + 10,
            dur: build_us,
            cpu: Some(build_us),
            node: 0,
            cut: false,
        }));
        t.records.push(Record::FrameEnd {
            t: at + 10 + build_us,
            frame: i + 1,
            unchanged: Some(false),
            nodes: 10,
        });
    }
    t
}

#[test]
fn every_record_reads_back_as_written() {
    let mut t = Trace::new();
    t.session = Some(Session {
        kui: "0.1.0".into(),
        app: "demo".into(),
        platform: "macos".into(),
        refresh_hz: 120.0,
        budget_ms: 8.33,
        scale: 2.0,
        wall_ms: 1_761_000_000_000,
    });
    let (a, _) = t.names.get("pointerMove");
    let (b, _) = t.names.get("parser");
    t.tracks.push((Track(2), b));
    let recs = vec![
        Record::FrameBegin {
            t: 5,
            frame: 1,
            causes: vec![a, b],
        },
        Record::Holder {
            frame: 1,
            what: a,
            name: b,
            node: 0xdead_beef,
        },
        Record::Span(Span {
            track: Track::MAIN,
            id: 9,
            parent: 3,
            name: b,
            origin: Origin::Extension(a),
            start: 2,
            dur: 70,
            cpu: None,
            node: 77,
            cut: true,
        }),
        Record::FrameEnd {
            t: 90,
            frame: 1,
            unchanged: Some(true),
            nodes: 120,
        },
        Record::Present {
            t: 95,
            frame: 1,
            render: 1500,
            wait: 6000,
        },
        Record::Input {
            t: 100,
            kind: a,
            os_t: Some(98),
            count: 14,
            until: 400,
        },
        Record::Event {
            t: 410,
            kind: a,
            node: 5,
        },
        Record::Wake { t: 420, name: b },
        Record::Request {
            t: 430,
            why: a,
            at: b,
        },
        Record::Loop {
            t: 440,
            event: LoopEvent::Sleep { until: Some(9000) },
        },
        Record::Loop {
            t: 450,
            event: LoopEvent::Woke { by: a },
        },
        Record::Loop {
            t: 460,
            event: LoopEvent::Os { kind: b },
        },
        Record::Counter {
            t: 470,
            frame: 1,
            name: a,
            value: -3,
        },
        Record::Gauge {
            t: 480,
            name: b,
            value: 1.5e9,
        },
        Record::Mark {
            t: 490,
            track: Track(2),
            name: a,
        },
        Record::Note {
            t: 3,
            text: "back in time".into(),
        },
        Record::Placement {
            t: 500,
            node: 0xfeed,
            component: a,
            answer: b,
            why: 0,
        },
    ];
    t.records = recs.clone();
    let back = Trace::from_bytes(&t.to_bytes()).expect("reads");
    assert_eq!(back.records, recs);
    assert_eq!(back.session, t.session);
    assert_eq!(back.name(b), "parser");
    assert_eq!(back.track_name(Track(2)), "parser");
}

#[test]
fn a_recording_cut_off_mid_record_reads_up_to_the_cut() {
    let t = frames(5, 1000, "wake");
    let b = t.to_bytes();
    let cut = Trace::from_bytes(&b[..b.len() - 3]).unwrap();
    assert_eq!(cut.records.len(), t.records.len() - 1);
    assert!(Trace::from_bytes(b"nope").is_err());
}

#[test]
fn a_tag_from_a_later_version_is_skipped() {
    let t = frames(1, 1000, "wake");
    let mut b = t.to_bytes();
    // A record no reader knows, then the rest again.
    let mut more = vec![200u8, 3, 1, 2, 3];
    more.extend_from_slice(&frames(1, 1000, "wake").to_bytes()[10..]);
    b.extend_from_slice(&more);
    let back = Trace::from_bytes(&b).unwrap();
    assert!(back.records.len() > t.records.len());
}

#[test]
fn a_run_of_frames_with_no_input_is_a_burn() {
    let t = frames(40, 500, "owed");
    let tl = t.timeline();
    let burns = analysis::burns(&tl, &t);
    assert_eq!(burns.len(), 1);
    assert_eq!((burns[0].first, burns[0].last), (0, 39));
    assert_eq!(
        analysis::verdict(&tl, 5, 8_000, &burns, &t),
        Verdict::Burn { k: 6, of: 40 }
    );
    // Answering keys, the same frames are no burn.
    let t = frames(40, 500, "key");
    assert!(analysis::burns(&t.timeline(), &t).is_empty());
}

#[test]
fn a_slow_frame_names_its_phase_and_its_span() {
    let t = frames(3, 20_000, "key");
    let tl = t.timeline();
    match analysis::verdict(&tl, 1, 8_000, &[], &t) {
        Verdict::Slow { phase, span, .. } => {
            assert_eq!(phase, "build");
            assert_eq!(t.name(span.unwrap().0), "build");
        }
        v => panic!("{v:?}"),
    }
}

#[test]
fn an_input_that_waited_for_its_frame_is_late_and_the_path_has_its_legs() {
    let mut t = frames(2, 500, "key");
    let (key, _) = t.names.get("key");
    let (handler, _) = t.names.get(analysis::HANDLER);
    // An input at 1 ms, its handler at 2–3 ms, the second frame at 16 ms.
    t.records.insert(
        3,
        Record::Input {
            t: 1_000,
            kind: key,
            os_t: None,
            count: 1,
            until: 1_000,
        },
    );
    t.records.insert(
        4,
        Record::Span(Span {
            track: Track::MAIN,
            id: 100,
            parent: 0,
            name: handler,
            origin: Origin::Host,
            start: 2_000,
            dur: 1_000,
            cpu: None,
            node: 0,
            cut: false,
        }),
    );
    let tl = t.timeline();
    let path = analysis::input_path(&tl, 1, &t).expect("the second frame answers it");
    assert_eq!(path.handler, Some((2_000, 3_000)));
    let legs = path.legs();
    assert_eq!(legs[0], ("to handler", 1_000));
    assert_eq!(legs[2], ("to frame", 13_000));
    assert!(matches!(
        analysis::verdict(&tl, 1, 8_000, &[], &t),
        Verdict::Late {
            wait: 15_000,
            slept: false
        }
    ));
    assert_eq!(analysis::phases(&tl, 1, &t).handler, 1_000);
}

#[test]
fn a_gap_with_no_sleep_is_the_thread_busy_elsewhere() {
    let mut t = frames(2, 500, "wake");
    // The loop recorded, but no sleep in the 15 ms gap — and a second
    // frame pushed 40 ms out.
    if let Record::FrameBegin { t: at, .. } = &mut t.records[3] {
        *at = 40_000;
    }
    t.records.push(Record::Loop {
        t: 600,
        event: LoopEvent::Woke { by: 0 },
    });
    let tl = Trace::from_bytes(&t.to_bytes()).unwrap().timeline();
    assert!(matches!(
        analysis::verdict(&tl, 1, 8_000, &[], &t),
        Verdict::Blocked { .. }
    ));
}

#[test]
fn spans_group_by_name_with_their_self_time() {
    let mut t = frames(2, 1_000, "key");
    let (rows, _) = t.names.get("rows");
    t.records.push(Record::Span(Span {
        track: Track::MAIN,
        id: 50,
        parent: 1,
        name: rows,
        origin: Origin::Host,
        start: 100,
        dur: 600,
        cpu: None,
        node: 0,
        cut: false,
    }));
    let tl = t.timeline();
    let stats = analysis::by_span(&tl, &tl.spans);
    let build = stats.iter().find(|s| t.name(s.name) == "build").unwrap();
    assert_eq!(build.calls, 2);
    assert_eq!(build.total, 2_000);
    assert_eq!(build.self_total, 1_400, "less its child's 600");
    let origins = analysis::by_origin(&tl, &tl.spans);
    assert_eq!(origins[0].0, Origin::Kui);
}

#[test]
fn the_dump_is_a_line_a_record() {
    let t = frames(2, 1_000, "key");
    let text = crate::dump::dump(&t);
    assert_eq!(text.lines().count(), t.records.len());
    assert!(text.contains("frame+ #1 key"), "{text}");
    assert!(text.contains("span main [kui] build 1.000 ms"), "{text}");
}

/// Placements group by kind: how many replayed, and the rest by answer
/// and reason, the kind drawn fresh most first.
#[test]
fn placements_group_by_kind_and_answer() {
    let mut t = frames(1, 1000, "key");
    let mut n = |s: &str| t.names.get(s).0;
    let (row, chip, replayed, reads, hover, params) = (
        n("ui:row"),
        n("ui:chip"),
        n(analysis::REPLAYED),
        n("reads"),
        n("hover 00000000000000ab"),
        n("params"),
    );
    let placed = |t: u64, component, answer, why| Record::Placement {
        t,
        node: 7,
        component,
        answer,
        why,
    };
    t.records.extend([
        placed(20, row, replayed, 0),
        placed(21, row, replayed, 0),
        placed(22, row, reads, hover),
        placed(23, chip, params, 0),
        placed(24, chip, params, 0),
        placed(25, chip, reads, hover),
    ]);
    let tl = t.timeline();
    assert_eq!(tl.placements_in(0, 23).len(), 3);
    let stats = analysis::placements(&tl, &t);
    assert_eq!(stats.len(), 2);
    assert_eq!((stats[0].component, stats[0].drawn()), (chip, 3));
    assert_eq!(stats[0].fresh[0], (params, 0, 2));
    assert_eq!(
        (stats[1].placed, stats[1].replayed, stats[1].fresh.clone()),
        (3, 2, vec![(reads, hover, 1)])
    );
    assert!(crate::dump::dump(&t).contains("placed ui:chip reads (hover 00000000000000ab) node"));
}

/// Five frames, each with a parent span written after its child (as a
/// recorder closes them), and a gauge in each gap.
fn nested(n: u64) -> Trace {
    let mut t = frames(n, 1000, "key");
    let (outer, _) = t.names.get("outer");
    let (inner, _) = t.names.get("inner");
    let (mem, _) = t.names.get("memory");
    for i in 0..n {
        let at = i * 16_000 + 100;
        let span = |id: u32, parent: u32, name, start: u64, dur: u64| Span {
            track: Track::MAIN,
            id,
            parent,
            name,
            origin: Origin::Host,
            start,
            dur,
            cpu: None,
            node: 0,
            cut: false,
        };
        let base = 1000 + i as u32 * 2;
        t.records
            .push(Record::Span(span(base + 1, base, inner, at + 50, 100)));
        t.records.push(Record::Span(span(base, 0, outer, at, 400)));
        t.records.push(Record::Gauge {
            t: at + 5000,
            name: mem,
            value: i as f64,
        });
    }
    t
}

/// A recording pushed a few bytes at a time reads as the whole file does,
/// its timeline as the whole one, a cut record waiting for its rest
/// (backlog F246).
#[test]
fn a_stream_pushed_in_pieces_reads_as_the_whole() {
    let t = nested(5);
    let bytes = t.to_bytes();
    let whole = Trace::from_bytes(&bytes).unwrap();
    for step in [1, 3, 7, 64, bytes.len()] {
        let mut s = crate::Stream::new();
        let mut came = 0;
        for piece in bytes.chunks(step) {
            came += s.push(piece).unwrap();
        }
        let names = whole.names.all().len() - 1;
        assert_eq!(
            came,
            whole.records.len() + names,
            "step {step}: and the names"
        );
        assert_eq!(s.trace().records, whole.records, "step {step}");
        assert_eq!(*s.timeline(), whole.timeline(), "step {step}");
        assert_eq!(s.pending(), 0);
    }
    let mut s = crate::Stream::new();
    assert!(s.push(b"KUI").is_ok(), "a header's start waits");
    assert!(s.push(b"nope, not a trace").is_err());
}

fn temp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("kui-trace-{}-{name}", std::process::id()));
    let _ = std::fs::remove_file(&dir);
    dir
}

/// The file as it is written, then written again from the start: what
/// grew, then a reset with the new recording whole; the file's end idle
/// while followed and ended when not.
#[test]
fn a_file_read_as_it_grows_and_again_when_rewritten() {
    use std::io::Write;
    let path = temp("grow.kuitrace");
    let bytes = nested(4).to_bytes();
    let half = bytes.len() / 2;
    std::fs::write(&path, &bytes[..half]).unwrap();
    let mut file = crate::FileTransport::open(&path, true).unwrap();
    let mut s = crate::Stream::new();
    assert!(matches!(s.poll(&mut file).unwrap(), crate::Polled::More(_)));
    assert_eq!(s.poll(&mut file).unwrap(), crate::Polled::Idle);
    std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(&bytes[half..])
        .unwrap();
    assert!(matches!(s.poll(&mut file).unwrap(), crate::Polled::More(_)));
    assert_eq!(s.timeline().frames.len(), 4);

    let other = nested(2).to_bytes();
    std::fs::write(&path, &other).unwrap();
    assert_eq!(s.poll(&mut file).unwrap(), crate::Polled::Reset);
    assert_eq!(s.timeline().frames.len(), 2);

    let mut once = crate::FileTransport::open(&path, false).unwrap();
    let mut s = crate::Stream::new();
    s.poll(&mut once).unwrap();
    assert_eq!(s.poll(&mut once).unwrap(), crate::Polled::Ended);
    let _ = std::fs::remove_file(&path);
}

/// A watch calls back when the file moves, and not while it stands still.
#[test]
fn a_watch_says_when_the_file_moved() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let path = temp("watch.kuitrace");
    std::fs::write(&path, b"KUITRACE").unwrap();
    let calls = std::sync::Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let watch = crate::Watch::spawn(&path, std::time::Duration::from_millis(10), move || {
        seen.fetch_add(1, Ordering::Relaxed);
    })
    .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(60));
    assert_eq!(calls.load(Ordering::Relaxed), 0, "still");
    std::fs::write(&path, b"KUITRACE and more").unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while calls.load(Ordering::Relaxed) == 0 && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    drop(watch);
    let _ = std::fs::remove_file(&path);
}

/// A made-up session is the same for a seed, and has what a real one has:
/// frames a span count apart, children inside their parents, burns,
/// slow frames, components drawn fresh, a gap the loop slept through
/// (backlog F247).
#[test]
fn a_made_up_session_has_a_sessions_shape() {
    use crate::synth::Synth;
    let s = Synth {
        frames: 3000,
        ..Default::default()
    };
    let t = s.trace();
    assert_eq!(t.to_bytes(), s.trace().to_bytes(), "the same seed");
    assert_ne!(
        t.to_bytes(),
        Synth { seed: 2, ..s }.trace().to_bytes(),
        "another seed"
    );
    let mut streamed = Vec::new();
    s.write(&mut streamed).unwrap();
    assert_eq!(Trace::from_bytes(&streamed).unwrap().records, t.records);

    let tl = t.timeline();
    assert_eq!(tl.frames.len(), 3000);
    let per = tl.spans.len() as f64 / 3000.0;
    assert!((10.0..20.0).contains(&per), "{per} spans a frame");
    let by_id: std::collections::HashMap<u32, &Span> = tl.spans.iter().map(|s| (s.id, s)).collect();
    for sp in &tl.spans {
        if let Some(p) = by_id.get(&sp.parent) {
            assert!(
                sp.start >= p.start && sp.end() <= p.end(),
                "{sp:?} in {p:?}"
            );
        }
    }
    assert!(!analysis::burns(&tl, &t).is_empty(), "burns");
    let budget = (1e6 / 120.0) as u64;
    let verdicts: Vec<Verdict> = (0..tl.frames.len())
        .map(|i| analysis::verdict(&tl, i, budget, &[], &t))
        .collect();
    assert!(
        verdicts.iter().any(|v| matches!(v, Verdict::Slow { .. })),
        "slow frames"
    );
    assert!(
        verdicts.iter().any(|v| matches!(v, Verdict::Fine { .. })),
        "fine ones"
    );
    let placed = analysis::placements(&tl, &t);
    assert!(placed.iter().any(|p| p.replayed > 0 && p.drawn() > 0));
    assert!(tl.loops.len() > 3000 && tl.inputs.len() > 500 && !tl.gauges.is_empty());
}
