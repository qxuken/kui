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
