use super::*;
use crate::{Record, Session, Track};

fn recording() -> Trace {
    let mut t = Trace::new();
    t.session = Some(Session {
        budget_ms: 1.0,
        ..Default::default()
    });
    let parent = t.names.get("layout").0;
    let child = t.names.get("child").0;
    let mark = t.names.get("measure").0;
    for (i, frame) in [100, 200, 300].into_iter().enumerate() {
        let begin = i as u64 * 2000;
        t.records.push(Record::FrameBegin {
            t: begin,
            frame,
            causes: vec![],
        });
        t.records.push(Record::Span(Span {
            track: Track::MAIN,
            id: i as u32 * 2 + 1,
            parent: 0,
            name: parent,
            origin: Origin::Host,
            start: begin + 100,
            dur: 1500,
            cpu: None,
            node: 0,
            cut: false,
        }));
        t.records.push(Record::Span(Span {
            track: Track::MAIN,
            id: i as u32 * 2 + 2,
            parent: i as u32 * 2 + 1,
            name: child,
            origin: Origin::Host,
            start: begin + 200,
            dur: 1000,
            cpu: Some(500),
            node: 0,
            cut: false,
        }));
        t.records.push(Record::Mark {
            t: begin + 300,
            track: Track::MAIN,
            name: mark,
        });
        t.records.push(Record::FrameEnd {
            t: begin + 2000,
            frame,
            unchanged: Some(i == 1),
            nodes: 2,
        });
        t.records.push(Record::Present {
            t: begin + 2000,
            frame,
            render: 0,
            wait: 0,
        });
    }
    t
}

#[test]
fn scope_expands_time_and_uses_recorded_numbers_without_boundary_overlap() {
    let t = recording();
    let tl = t.timeline();
    let ex = Explorer::new(&t, &tl);
    let f = Filter::default();
    let s = ex
        .resolve(
            &Selection::Time {
                from_us: 500,
                to_us: 600,
            },
            &f,
        )
        .unwrap();
    assert_eq!(s.indices, vec![0]);
    assert_eq!(s.intervals, vec![(0, 2000)]);
    let s = ex
        .resolve(
            &Selection::Time {
                from_us: 2000,
                to_us: 3000,
            },
            &f,
        )
        .unwrap();
    assert_eq!(s.indices, vec![1]);
    let s = ex
        .resolve(
            &Selection::Frames {
                first: 200,
                last: 200,
            },
            &f,
        )
        .unwrap();
    assert_eq!(s.indices, vec![1]);
    assert_eq!(s.intervals, vec![(2000, 4000)]);
    assert!(
        ex.resolve(&Selection::Frames { first: 0, last: 2 }, &f)
            .unwrap()
            .indices
            .is_empty()
    );
    assert!(
        ex.resolve(
            &Selection::Time {
                from_us: 8000,
                to_us: 9000
            },
            &f
        )
        .unwrap()
        .intervals
        .is_empty()
    );
    assert!(
        ex.resolve(&Selection::Frames { first: 3, last: 2 }, &f)
            .is_err()
    );
}

#[test]
fn attribution_does_not_change_self_cost_or_frame_denominators() {
    let t = recording();
    let tl = t.timeline();
    let ex = Explorer::new(&t, &tl);
    let f = Filter {
        name: Some("LAYOUT".into()),
        ..Default::default()
    };
    let s = ex.resolve(&Selection::Whole, &f).unwrap();
    let groups = ex.span_groups(&s, &f);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].self_us, 1500);
    assert_eq!(groups[0].total_us, 4500);
    assert_eq!(ex.summary(&s).frames, 3);
    assert_eq!(ex.summary(&s).work.avg_us, Some(2000.0));
    assert_eq!(
        groups[0].cpu_us, None,
        "partial CPU availability must not look like a full measurement"
    );
    let filter = Filter {
        hide_unchanged: true,
        ..f
    };
    let s = ex.resolve(&Selection::Whole, &filter).unwrap();
    assert_eq!(s.indices, vec![0, 2]);
    assert_eq!(ex.span_groups(&s, &filter)[0].self_us, 1000);
    assert_eq!(
        ex.summary(&s).pacing.samples,
        0,
        "do not measure pacing across omitted frames"
    );
}

#[test]
fn selected_frame_population_excludes_spans_in_an_overlapping_present() {
    let mut trace = Trace::new();
    let work = trace.names.get("background").0;
    let tail = trace.names.get("unframed tail").0;
    for (frame, begin, end, present, unchanged) in
        [(1, 0, 100, 200, false), (2, 300, 400, 500, true)]
    {
        trace.records.extend([
            Record::FrameBegin {
                t: begin,
                frame,
                causes: vec![],
            },
            Record::FrameEnd {
                t: end,
                frame,
                unchanged: Some(unchanged),
                nodes: 0,
            },
            Record::Present {
                t: present,
                frame,
                render: 20,
                wait: 80,
            },
        ]);
    }
    for (id, name, start) in [(1, work, 150), (2, tail, 550)] {
        trace.records.push(Record::Span(Span {
            track: Track(2),
            id,
            parent: 0,
            name,
            origin: Origin::Host,
            start,
            dur: 20,
            cpu: None,
            node: 0,
            cut: false,
        }));
    }
    let tl = trace.timeline();
    let ex = Explorer::new(&trace, &tl);
    assert_eq!(ex.span_detail(1).unwrap().frame, Some(2));
    assert_eq!(ex.span_detail(2).unwrap().frame, None);
    let mut q = Query::new(Operation::Spans);
    q.filter.hide_unchanged = true;
    let result = ex.execute(&q).unwrap();
    assert_eq!(result.scope.indices, vec![0]);
    assert_eq!(
        result.matched, 0,
        "frame 2's gap must not leak through frame 1's present"
    );
    assert!(ex.origins(&result.scope, &q.filter).is_empty());
    q.filter = Filter::default();
    q.selection = Selection::Frames { first: 1, last: 1 };
    assert_eq!(
        ex.execute(&q).unwrap().matched,
        0,
        "explicit frame ranges honor ownership too"
    );
    q.selection = Selection::Frames { first: 2, last: 2 };
    let Data::Spans(groups) = ex.execute(&q).unwrap().data else {
        panic!()
    };
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].name, work);
    assert_eq!(groups[0].total_us, 20);
    q.selection = Selection::Whole;
    assert_eq!(
        ex.execute(&q).unwrap().matched,
        2,
        "whole recordings retain unframed work"
    );
}

#[test]
fn detail_references_survive_filtering_and_sorting() {
    let t = recording();
    let tl = t.timeline();
    let ex = Explorer::new(&t, &tl);
    let mut q = Query::new(Operation::Spans);
    q.filter.name = Some("nothing".into());
    q.selection = Selection::Frames {
        first: 200,
        last: 200,
    };
    assert_eq!(ex.execute(&q).unwrap().matched, 0);
    q.operation = Operation::Span(1);
    let Data::Span(s) = ex.execute(&q).unwrap().data else {
        panic!()
    };
    assert_eq!(s.span.id, 1);
    assert_eq!(s.frame, Some(100));
    assert_eq!(s.children, vec![2]);
    assert_eq!(s.self_us, 500);
    assert_eq!(ex.span_detail(2).unwrap().depth, 1);
    assert_eq!(ex.frame_detail(1).frame.frame, 200);
    assert!(ex.execute(&Query::new(Operation::Span(999))).is_err());
}

#[test]
fn result_limits_and_ties_are_explicit_and_deterministic() {
    let mut t = recording();
    for i in 0..47 {
        let name = t.names.get(&format!("group-{i:02}")).0;
        t.records.push(Record::Span(Span {
            track: Track::MAIN,
            id: 100 + i,
            parent: 0,
            name,
            origin: Origin::Host,
            start: 10,
            dur: 10,
            cpu: None,
            node: 0,
            cut: false,
        }));
    }
    let tl = t.timeline();
    let ex = Explorer::new(&t, &tl);
    let mut q = Query::new(Operation::Spans);
    let r = ex.execute(&q).unwrap();
    assert_eq!(r.matched, 49);
    assert_eq!(r.returned, 20);
    let Data::Spans(ss) = r.data else { panic!() };
    assert_eq!(t.name(ss[2].name), "group-00");
    assert_eq!(t.name(ss[19].name), "group-17");
    q.limit = None;
    assert_eq!(ex.execute(&q).unwrap().returned, 49);
    q.limit = Some(0);
    let r = ex.execute(&q).unwrap();
    assert_eq!(r.matched, 49);
    assert_eq!(r.returned, 0);
    q.sort = "unknown".into();
    assert!(ex.execute(&q).is_err());
}

#[test]
fn repeated_marks_are_events_not_implicit_regions() {
    let t = recording();
    let tl = t.timeline();
    let ex = Explorer::new(&t, &tl);
    let r = ex.execute(&Query::new(Operation::Marks)).unwrap();
    let Data::Marks(ms) = r.data else { panic!() };
    assert_eq!(ms.len(), 3);
    assert_eq!(ms[0].name, ms[2].name);
    assert!(ms[0].t < ms[2].t);
}

#[test]
fn comparison_keeps_independent_populations_and_missing_values() {
    let a = recording();
    let mut b = recording();
    let removed = b.names.get("child").0;
    b.records
        .retain(|r| !matches!(r,Record::Span(s) if s.name==removed));
    let atl = a.timeline();
    let btl = b.timeline();
    let ax = Explorer::new(&a, &atl);
    let bx = Explorer::new(&b, &btl);
    let mut aq = Query::new(Operation::Summary);
    aq.selection = Selection::Frames {
        first: 100,
        last: 100,
    };
    let mut bq = Query::new(Operation::Summary);
    bq.selection = Selection::Frames {
        first: 200,
        last: 300,
    };
    let c = compare(&ax, &aq, &bx, &bq, None).unwrap();
    let count = c.rows.iter().find(|r| r.metric == "frames").unwrap();
    assert_eq!(count.before, Some(1.0));
    assert_eq!(count.after, Some(2.0));
    let child = c
        .rows
        .iter()
        .find(|r| r.metric.contains("child") && r.metric.ends_with(".self"))
        .unwrap();
    assert!(child.before.is_some());
    assert_eq!(child.after, None);
    assert_eq!(child.percent, None);
    let zeros = c.rows.iter().find(|r| r.metric == "unchanged").unwrap();
    assert_eq!(zeros.before, Some(0.0));
    assert_eq!(zeros.percent, None);
    assert_eq!(c.before.scope.indices, vec![0]);
    assert_eq!(c.after.scope.indices, vec![1, 2]);
}

#[test]
fn incomplete_and_empty_recordings_do_not_invent_timings_or_budget() {
    let mut t = Trace::new();
    t.records.push(Record::FrameBegin {
        t: 100,
        frame: 9,
        causes: vec![],
    });
    let name = t.names.get("unfinished").0;
    t.records.push(Record::Span(Span {
        track: Track::MAIN,
        id: 1,
        parent: 0,
        name,
        origin: Origin::Host,
        start: 200,
        dur: 100,
        cpu: None,
        node: 0,
        cut: false,
    }));
    let tl = t.timeline();
    let ex = Explorer::new(&t, &tl);
    let r = ex.execute(&Query::new(Operation::Summary)).unwrap();
    let Data::Summary(s) = r.data else { panic!() };
    assert_eq!(s.frames, 1);
    assert_eq!(s.complete_frames, 0);
    assert_eq!(s.work.avg_us, None);
    assert_eq!(s.budget_us, None);
    assert_eq!(ex.frame_detail(0).verdict, None);
    assert_eq!(ex.span_detail(1).unwrap().frame, Some(9));
    let scope = ex
        .resolve(&Selection::Frames { first: 9, last: 9 }, &Filter::default())
        .unwrap();
    assert!(scope.contains(200));
    assert_eq!(ex.span_groups(&scope, &Filter::default()).len(), 1);

    let t = Trace::new();
    let tl = t.timeline();
    let ex = Explorer::new(&t, &tl);
    let s = ex.resolve(&Selection::Whole, &Filter::default()).unwrap();
    assert!(s.intervals.is_empty());
    assert_eq!(ex.summary(&s).work.samples, 0);
}

#[test]
fn missing_present_keeps_work_unavailable_in_details_ranking_and_comparison() {
    let mut before = Trace::new();
    before.session = Some(Session {
        budget_ms: 1.0,
        ..Default::default()
    });
    before.records.extend([
        Record::FrameBegin {
            t: 0,
            frame: 10,
            causes: vec![],
        },
        Record::FrameEnd {
            t: 100,
            frame: 10,
            unchanged: Some(false),
            nodes: 0,
        },
    ]);
    let mut after = before.clone();
    after.records.push(Record::Present {
        t: 200,
        frame: 10,
        render: 100,
        wait: 0,
    });
    let btl = before.timeline();
    let atl = after.timeline();
    let bx = Explorer::new(&before, &btl);
    let ax = Explorer::new(&after, &atl);
    let q = Query::new(Operation::Summary);
    let Data::Summary(summary) = bx.execute(&q).unwrap().data else {
        panic!()
    };
    assert_eq!(
        summary.complete_frames, 0,
        "FrameEnd alone does not provide the driver's work"
    );
    assert_eq!(summary.work, Distribution::default());
    assert_eq!(summary.over_budget, None);
    assert_eq!(
        bx.frame_detail(0).frame.core_us(),
        100,
        "known core time stays available"
    );
    assert!(bx.frame_detail(0).verdict.is_none());
    #[cfg(feature = "value")]
    {
        let value = crate::query_value::frame(&before, &bx.frame_detail(0));
        assert_eq!(value.get("work"), Some(&kui_value::Value::Null));
        assert_eq!(
            value.get("phases").unwrap().get("render"),
            Some(&kui_value::Value::Null)
        );
        assert_eq!(value.get_float("finish"), Some(0.1));
    }
    let compared = compare(&bx, &q, &ax, &q, None).unwrap();
    let work = compared
        .rows
        .iter()
        .find(|r| r.metric == "work.avg")
        .unwrap();
    assert_eq!(work.before, None);
    assert_eq!(work.after, Some(0.2));
    assert_eq!(work.delta, None);
    assert_eq!(
        work.percent, None,
        "missing rendering must not create a 100% regression"
    );

    let mut mixed = before.clone();
    mixed.records.extend([
        Record::FrameBegin {
            t: 300,
            frame: 20,
            causes: vec![],
        },
        Record::FrameEnd {
            t: 400,
            frame: 20,
            unchanged: Some(false),
            nodes: 0,
        },
        Record::Present {
            t: 500,
            frame: 20,
            render: 100,
            wait: 0,
        },
    ]);
    let tl = mixed.timeline();
    let ex = Explorer::new(&mixed, &tl);
    for descending in [true, false] {
        let mut q = Query::new(Operation::Frames);
        q.descending = descending;
        let Data::Frames(frames) = ex.execute(&q).unwrap().data else {
            panic!()
        };
        assert_eq!(
            frames.iter().map(|f| f.frame.frame).collect::<Vec<_>>(),
            vec![20, 10],
            "unavailable work never ranks as fastest or slowest"
        );
    }
    let filter = Filter {
        verdicts: Some(vec!["fine".into()]),
        ..Default::default()
    };
    assert_eq!(
        ex.resolve(&Selection::Whole, &filter).unwrap().indices,
        vec![1]
    );
}

#[test]
fn selector_parsing_rejects_nonfinite_negative_and_unitless_times() {
    assert_eq!(
        time_range("100us:2ms").unwrap(),
        Selection::Time {
            from_us: 100,
            to_us: 2000
        }
    );
    for s in ["0:1", "NaNs:2s", "-1ms:2ms", "1s:1s", "infs:2s", "2s:1s"] {
        assert!(time_range(s).is_err(), "{s}");
    }
    assert!(frame_range("3:2").is_err());
}
