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
