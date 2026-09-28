//! A table's `rules` (backlog DX21): grid lines down the middle of the gaps
//! its layout already has. kawoosh's markdown tables built `2n + 1` cells a
//! row, every other one a 1 px rule, plus an edge row, to draw them.

use kui_core::{Color, Core, NodeSpec, QuadKind, Rect, Size, TextStyle};

const VIEW: Size = Size { w: 300.0, h: 200.0 };
const RULE: Color = Color {
    r: 1.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};

/// A 200 px table, gap 4 between rows and between cells, padding 10: a
/// two-cell header and two three-cell rows of fixed cells, so every edge
/// is known.
fn rules(scale: f32, spec: NodeSpec) -> Vec<Rect> {
    let mut core = Core::new();
    let mut ui = core.frame(VIEW, scale);
    ui.with(spec.width(200.0).pad(10.0).gap(4.0), |ui| {
        ui.with(NodeSpec::row().gap(4.0), |ui| {
            ui.text_in(NodeSpec::row().width(40.0), "a", TextStyle::new(10.0));
            ui.text_in(NodeSpec::row().width(40.0), "b", TextStyle::new(10.0));
        });
        for _ in 0..2 {
            ui.with(NodeSpec::row().gap(4.0), |ui| {
                for w in [40.0, 40.0, 60.0] {
                    ui.leaf(NodeSpec::row().size(w, 20.0));
                }
            });
        }
    });
    ui.finish();
    core.output()
        .0
        .quads
        .iter()
        .filter(|q| q.kind == QuadKind::Solid && q.color == RULE)
        .map(|q| q.rect)
        .collect()
}

#[test]
fn the_rules_run_down_the_middle_of_every_gap() {
    let lines = rules(1.0, NodeSpec::table().rules(RULE));
    assert_eq!(
        lines.len(),
        4,
        "two between the three rows, two between the three columns"
    );
    // On whole pixels, so each edge is within half a pixel of the middle.
    let near = |a: f32, b: f32| assert!((a - b).abs() <= 0.5, "{a} vs {b}");
    let mid = |r: Rect, across: bool| {
        if across {
            r.y + r.h / 2.0
        } else {
            r.x + r.w / 2.0
        }
    };
    // Across: the content box wide (10..190), 1 px, in the 4 px gaps:
    // the second 24 px (a 20 px row and its gap) below the first.
    for l in &lines[..2] {
        assert_eq!((l.x, l.w, l.h), (10.0, 180.0, 1.0));
    }
    near(mid(lines[1], true) - mid(lines[0], true), 24.0);
    // Down: after the 40 px columns, mid-gap at 52 and 96, from the first
    // row's top to the last row's bottom — 22 px under the second rule.
    near(mid(lines[2], false), 52.0);
    near(mid(lines[3], false), 96.0);
    for l in &lines[2..] {
        assert_eq!((l.y, l.w), (10.0, 1.0));
        near(l.y + l.h, mid(lines[1], true) + 22.0);
    }
}

#[test]
fn the_width_holds_and_only_a_table_draws_them() {
    let lines = rules(1.0, NodeSpec::table().rules(RULE).rule_width(2.0));
    assert!(lines.iter().all(|l| l.w == 2.0 || l.h == 2.0), "{lines:?}");
    assert!(
        rules(1.0, NodeSpec::column().rules(RULE)).is_empty(),
        "a column is no table"
    );
    // At 1.5x the lines land on whole physical pixels.
    for l in rules(1.5, NodeSpec::table().rules(RULE)) {
        for v in [l.x, l.y, l.w, l.h] {
            assert_eq!(v, v.round(), "{l:?}");
        }
    }
}

/// The rule quads of the last frame, each cut to the clip it was pushed
/// with: what a renderer actually draws of it.
fn drawn_rules(core: &mut Core) -> Vec<Rect> {
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .filter(|q| q.kind == QuadKind::Solid && q.color == RULE)
        .map(|q| q.rect.intersect(&dl.clip_of(q).rect))
        .filter(|r| r.w > 0.0 && r.h > 0.0)
        .collect()
}

fn inside(r: Rect, outer: Rect) -> bool {
    r.x >= outer.x
        && r.y >= outer.y
        && r.x + r.w <= outer.x + outer.w
        && r.y + r.h <= outer.y + outer.h
}

/// The alpha.22 regression pass: a scrolled table's rules come from its
/// rows' laid-out places, which carry the scroll offset, and were pushed
/// with the clip around the table rather than the one it gives its rows —
/// scrolled 60 px, a row rule was drawn over the header above it.
#[test]
fn a_scrolled_tables_rules_are_clipped_to_it() {
    let mut core = Core::new();
    let frame = |core: &mut Core| {
        let mut ui = core.frame(VIEW, 1.0);
        let mut table = None;
        ui.with(NodeSpec::column(), |ui| {
            ui.leaf(NodeSpec::row().size(300.0, 50.0));
            table = Some(
                ui.with_keyed(
                    "table",
                    NodeSpec::table()
                        .scroll_y()
                        .size(200.0, 100.0)
                        .gap(4.0)
                        .rules(RULE),
                    |ui| {
                        for _ in 0..10 {
                            ui.with(NodeSpec::row().gap(4.0), |ui| {
                                ui.leaf(NodeSpec::row().size(40.0, 20.0));
                                ui.leaf(NodeSpec::row().size(40.0, 20.0));
                            });
                        }
                    },
                ),
            );
        });
        ui.finish();
        table.unwrap()
    };
    let table = frame(&mut core);
    let bounds = Rect::new(0.0, 50.0, 200.0, 100.0);
    let unscrolled = drawn_rules(&mut core);
    assert!(!unscrolled.is_empty());
    assert!(
        unscrolled.iter().all(|&r| inside(r, bounds)),
        "{unscrolled:?}"
    );
    core.set_scroll(table, kui_core::Vec2::new(0.0, 60.0));
    frame(&mut core);
    let scrolled = drawn_rules(&mut core);
    assert!(scrolled.len() > 2, "rules still drawn: {scrolled:?}");
    for r in scrolled {
        assert!(
            inside(r, bounds),
            "{r:?} drawn outside the table {bounds:?}"
        );
    }
}

/// A table that fades out on removal: `present` false drops it. The
/// place and alpha of each rule quad of the frame.
fn departing(core: &mut Core, t: f64, present: bool) -> Vec<(Rect, f32)> {
    core.set_time(t);
    let mut ui = core.frame(VIEW, 1.0);
    if present {
        ui.with_keyed(
            "table",
            NodeSpec::table()
                .width(200.0)
                .gap(4.0)
                .rules(RULE)
                .transition(200.0)
                .exit(kui_core::Enter::default().opacity(0.0)),
            |ui| {
                for _ in 0..3 {
                    ui.with(NodeSpec::row().gap(4.0), |ui| {
                        ui.leaf(NodeSpec::row().size(40.0, 20.0));
                        ui.leaf(NodeSpec::row().size(40.0, 20.0));
                    });
                }
            },
        );
    }
    ui.finish();
    core.output()
        .0
        .quads
        .iter()
        .filter(|q| {
            q.kind == QuadKind::Solid && (q.color.r, q.color.g, q.color.b) == (1.0, 0.0, 0.0)
        })
        .map(|q| (q.rect, q.color.a))
        .collect()
}

/// The alpha.22 regression pass: the ghost of a departing table kept its
/// spec and painted only its boxes, so its rules went on the first frame
/// of the fade.
#[test]
fn a_departing_table_fades_its_rules_with_it() {
    let mut core = Core::new();
    let live = departing(&mut core, 0.0, true);
    assert_eq!(live.len(), 3, "two across, one down: {live:?}");
    assert!(live.iter().all(|&(_, a)| a == 1.0));
    departing(&mut core, 0.0, true);
    let first = departing(&mut core, 0.016, false);
    let places = |v: &[(Rect, f32)]| v.iter().map(|&(r, _)| r).collect::<Vec<_>>();
    assert_eq!(
        places(&first),
        places(&live),
        "the ghost draws its rules where they were"
    );
    let mid = departing(&mut core, 0.1, false);
    assert_eq!(places(&mid), places(&live));
    assert!(
        mid.iter().all(|&(_, a)| a > 0.0 && a < 1.0),
        "faded with the ghost: {mid:?}"
    );
    assert!(
        departing(&mut core, 0.5, false).is_empty(),
        "gone once it has"
    );
}

const SECTION: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 1.0,
    a: 1.0,
};

/// The alpha.22 regression pass: the rules took any in-flow `row` child
/// of a table for a row, where layout takes an in-flow `row` container
/// (RG7) — a `column` section wrapping a heading over a row had the
/// column rule drawn through its heading, and a `row` image beside the
/// rows counted as a row of no cells, the column rule running over it.
/// A child that is no row reads as one spanning every column: ruled off
/// above and below, and the column rules stop at it.
#[test]
fn the_rules_go_around_a_child_that_is_no_row() {
    let mut core = Core::new();
    let banner = core.resources.add_image(4, 4, vec![0x80; 64]);
    let mut ui = core.frame(VIEW, 1.0);
    let row = |ui: &mut kui_core::Ui<'_>| {
        ui.with(NodeSpec::row().gap(4.0), |ui| {
            ui.leaf(NodeSpec::row().size(40.0, 20.0));
            ui.leaf(NodeSpec::row().size(40.0, 20.0));
        });
    };
    ui.with(
        NodeSpec::table()
            .width(200.0)
            .pad(10.0)
            .gap(4.0)
            .rules(RULE),
        |ui| {
            row(ui);
            ui.with(NodeSpec::column().bg(SECTION), |ui| {
                ui.text("Section", TextStyle::new(10.0));
                row(ui);
            });
            row(ui);
            ui.image(banner, NodeSpec::row().size(180.0, 14.0));
            row(ui);
        },
    );
    ui.finish();
    let (dl, _) = core.output();
    let rect_of = |kind: QuadKind, c: Option<Color>| {
        dl.quads
            .iter()
            .find(|q| q.kind == kind && c.is_none_or(|c| q.color == c))
            .map(|q| q.rect)
            .expect("drawn")
    };
    let section = rect_of(QuadKind::Solid, Some(SECTION));
    let image = rect_of(QuadKind::Image, None);
    let lines: Vec<Rect> = dl
        .quads
        .iter()
        .filter(|q| q.kind == QuadKind::Solid && q.color == RULE)
        .map(|q| q.rect)
        .collect();
    for l in &lines {
        for (name, r) in [("section", section), ("image", image)] {
            let x = l.intersect(&r);
            assert!(x.w <= 0.0 || x.h <= 0.0, "{l:?} crosses the {name} {r:?}");
        }
    }
    let (across, down): (Vec<Rect>, Vec<Rect>) = lines.iter().partition(|l| l.w > l.h);
    assert_eq!(
        across.len(),
        4,
        "one in each gap between the five: {across:?}"
    );
    assert_eq!(down.len(), 3, "one down each of the three rows: {down:?}");
    for d in &down {
        assert_eq!(d.h, 20.0, "a row high: {d:?}");
    }
}
