//! A memo (backlog F285): a part of the view built once and pushed again
//! while its deps hold, on the journal a component instance replays by,
//! with no extension behind it. What it pushes again is what it built; its
//! events, labels and focus are the view's own; a memo inside a memo is
//! kept on its own.

use std::cell::Cell;

use kui_core::testing::click_at;
use kui_core::{
    Core, FillAnswer, InputEvent, Key, NodeSpec, Size, TextStyle, Ui, UiEvent, Value, Vec2,
};

const VIEW: Size = Size { w: 400.0, h: 300.0 };

/// A list of `rows` text rows, each a clickable box, counting the builds.
fn rows(ui: &mut Ui<'_>, rows: i64, built: &Cell<usize>) {
    built.set(built.get() + 1);
    for i in 0..rows {
        ui.with_keyed(
            &format!("row{i}"),
            NodeSpec::row().size(200.0, 20.0).on_click(Value::map([
                ("kind", Value::str("row")),
                ("row", Value::Int(i)),
            ])),
            |ui| ui.text(&format!("row {i}"), TextStyle::new(12.0)),
        );
    }
}

/// One frame: a box before, the memo, a box after (auto-keyed, so its key
/// says whether the memo moved its siblings' counter). Returns the memo's
/// answer and the after box's key.
fn frame(core: &mut Core, deps: i64, built: &Cell<usize>) -> (FillAnswer, Key) {
    let mut ui = core.frame(VIEW, 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.leaf(NodeSpec::row().size(10.0, 10.0));
    let answer = ui.memo("list", &Value::Int(deps), |ui| rows(ui, deps, built));
    let after = ui.leaf(NodeSpec::row().size(10.0, 10.0));
    ui.finish();
    (answer, after)
}

/// A core that keeps each frame's nodes for `nodes()`.
fn new_core() -> Core {
    let mut core = Core::new();
    core.set_inspect(true);
    core
}

fn snapshot(core: &mut Core) -> Vec<(Key, kui_core::Rect)> {
    core.nodes().iter().map(|n| (n.key, n.rect)).collect()
}

#[test]
fn a_memo_is_pushed_again_while_its_deps_hold() {
    let mut core = new_core();
    let built = Cell::new(0);
    let (first, after) = frame(&mut core, 3, &built);
    assert_eq!(first, FillAnswer::NotKept);
    let fresh = snapshot(&mut core);
    let quads = core.drawn().0.quads.len();

    let (again, after_again) = frame(&mut core, 3, &built);
    assert_eq!(again, FillAnswer::Replayed);
    assert_eq!(built.get(), 1, "the body ran once");
    assert_eq!(snapshot(&mut core), fresh, "the same nodes, keys and rects");
    assert_eq!(core.drawn().0.quads.len(), quads);
    assert_eq!(after, after_again, "a sibling's auto key is where it was");

    // The deps moved: built again, the new rows drawn.
    assert_eq!(frame(&mut core, 4, &built).0, FillAnswer::Params);
    assert_eq!(built.get(), 2);
    assert!(core.key_of("row3").is_some(), "the new row is declared");
    assert_eq!(frame(&mut core, 4, &built).0, FillAnswer::Replayed);
}

#[test]
fn a_memo_not_placed_for_a_frame_forgets_what_it_kept() {
    let mut core = new_core();
    let built = Cell::new(0);
    frame(&mut core, 2, &built);
    core.frame(VIEW, 1.0).finish();
    assert_eq!(frame(&mut core, 2, &built).0, FillAnswer::NotKept);
    assert_eq!(built.get(), 2);
}

#[test]
fn a_memo_is_built_again_when_what_it_read_moves() {
    let mut core = new_core();
    let built = Cell::new(0);
    let hovered = Cell::new(false);
    let run = |core: &mut Core| {
        let mut ui = core.frame(VIEW, 1.0);
        ui.configure_root(NodeSpec::column().fill());
        let a = ui.memo("m", &Value::Null, |ui| {
            built.set(built.get() + 1);
            let k = ui.child_key("hot");
            hovered.set(ui.is_hovered(k));
            ui.leaf_keyed(
                "hot",
                NodeSpec::row()
                    .size(100.0, 40.0)
                    .hover_bg(kui_core::Color::WHITE),
            );
        });
        ui.finish();
        a
    };
    run(&mut core);
    assert_eq!(run(&mut core), FillAnswer::Replayed);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(5.0, 5.0)));
    assert_eq!(run(&mut core), FillAnswer::Reads);
    assert!(hovered.get());
    assert_eq!(run(&mut core), FillAnswer::Replayed);
    assert_eq!(built.get(), 2);
}

#[test]
fn a_click_inside_a_memo_is_the_views_own_event() {
    let mut core = new_core();
    let built = Cell::new(0);
    frame(&mut core, 3, &built);
    frame(&mut core, 3, &built);
    // Row 1 is at y 10 (the box before) + 20.
    let events: Vec<UiEvent> = click_at(&mut core, 50.0, 35.0);
    let click = events
        .iter()
        .find(|e| e.payload.get_str("kind") == Some("row"))
        .expect("the row's click");
    assert_eq!(click.payload.get_int("row"), Some(1));
    assert_eq!(click.instance, None, "no instance: a memo is no component");
    assert_eq!(click.origin, kui_core::OriginId::HOST);
}

#[test]
fn a_label_inside_a_memo_is_found_as_the_views_own() {
    let mut core = new_core();
    let built = Cell::new(0);
    frame(&mut core, 2, &built);
    let fresh = core.key_of("row1");
    frame(&mut core, 2, &built);
    assert!(fresh.is_some());
    assert_eq!(core.key_of("row1"), fresh, "noted again by the replay");
}

/// An outer memo over two sections, each a memo of its own.
fn nested(
    core: &mut Core,
    outer: i64,
    a: i64,
    b: i64,
    built: &[Cell<usize>; 3],
) -> [FillAnswer; 3] {
    let mut ui = core.frame(VIEW, 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let inner = [Cell::new(None), Cell::new(None)];
    let o = ui.memo(
        "screen",
        &Value::list([Value::Int(outer), Value::Int(a), Value::Int(b)]),
        |ui| {
            built[0].set(built[0].get() + 1);
            inner[0].set(Some(
                ui.memo("a", &Value::Int(a), |ui| rows(ui, a, &built[1])),
            ));
            inner[1].set(Some(
                ui.memo("b", &Value::Int(b), |ui| rows(ui, b, &built[2])),
            ));
        },
    );
    ui.finish();
    // A section the outer replayed was not asked: it rode along.
    [
        o,
        inner[0].get().unwrap_or(FillAnswer::Replayed),
        inner[1].get().unwrap_or(FillAnswer::Replayed),
    ]
}

#[test]
fn a_memo_inside_a_memo_is_kept_on_its_own() {
    use FillAnswer::*;
    let mut core = new_core();
    let built = [Cell::new(0), Cell::new(0), Cell::new(0)];
    let counts = |b: &[Cell<usize>; 3]| [b[0].get(), b[1].get(), b[2].get()];

    assert_eq!(
        nested(&mut core, 0, 2, 3, &built),
        [NotKept, NotKept, NotKept]
    );
    let fresh = snapshot(&mut core);
    // Nothing moved: the outer replays, and the sections with it.
    assert_eq!(
        nested(&mut core, 0, 2, 3, &built),
        [Replayed, Replayed, Replayed]
    );
    assert_eq!(counts(&built), [1, 1, 1]);
    assert_eq!(snapshot(&mut core), fresh);
    // One section's deps moved: the outer is built again, the other
    // section replayed inside it.
    assert_eq!(
        nested(&mut core, 0, 2, 4, &built),
        [Params, Replayed, Params]
    );
    assert_eq!(counts(&built), [2, 1, 2]);
    // And kept again whole: the next frame replays all three.
    let moved = snapshot(&mut core);
    assert_eq!(
        nested(&mut core, 0, 2, 4, &built),
        [Replayed, Replayed, Replayed]
    );
    assert_eq!(snapshot(&mut core), moved);
    // The outer's own deps moved: both sections replay inside it.
    assert_eq!(
        nested(&mut core, 1, 2, 4, &built),
        [Params, Replayed, Replayed]
    );
    assert_eq!(counts(&built), [3, 1, 2]);
    assert_eq!(snapshot(&mut core), moved);
}

#[test]
fn a_read_inside_an_inner_memo_builds_the_outer_again() {
    let mut core = new_core();
    let views = [Cell::new(0), Cell::new(0)];
    let run = |core: &mut Core| {
        let mut ui = core.frame(VIEW, 1.0);
        ui.configure_root(NodeSpec::column().fill());
        let mut inner = None;
        let o = ui.memo("outer", &Value::Null, |ui| {
            views[0].set(views[0].get() + 1);
            inner = Some(ui.memo("inner", &Value::Null, |ui| {
                views[1].set(views[1].get() + 1);
                let k = ui.child_key("hot");
                let hot = ui.is_hovered(k);
                ui.leaf_keyed(
                    "hot",
                    NodeSpec::row()
                        .size(100.0, 40.0)
                        .hover_bg(kui_core::Color::WHITE),
                );
                ui.text(if hot { "hot" } else { "cold" }, TextStyle::new(12.0));
            }));
        });
        ui.finish();
        (o, inner)
    };
    run(&mut core);
    assert_eq!(run(&mut core), (FillAnswer::Replayed, None));
    core.handle_input(InputEvent::CursorMoved(Vec2::new(5.0, 5.0)));
    // The hover the inner read moved: the outer is built, and the inner
    // in it.
    assert_eq!(run(&mut core), (FillAnswer::Reads, Some(FillAnswer::Reads)));
    assert_eq!((views[0].get(), views[1].get()), (2, 2));
    assert!(
        core.access_tree()
            .nodes
            .iter()
            .any(|n| n.name.as_deref() == Some("hot"))
    );
    assert_eq!(run(&mut core), (FillAnswer::Replayed, None));
}

#[test]
fn a_memo_left_open_is_closed_and_said() {
    let mut core = new_core();
    let mut ui = core.frame(VIEW, 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.memo("m", &Value::Null, |ui| {
        ui.open(NodeSpec::column());
    });
    let after = ui.leaf(NodeSpec::row().size(10.0, 10.0));
    ui.finish();
    let warnings = core.take_warnings();
    assert!(
        warnings.iter().any(|w| w.code == "unclosed-node"),
        "{warnings:?}"
    );
    let nodes = core.nodes();
    let after = nodes.iter().find(|n| n.key == after).expect("after");
    assert_eq!(after.depth, 1, "the box after the memo is the root's child");
}

#[test]
fn a_memo_indexed_keeps_each_row_as_the_list_shifts() {
    let mut core = new_core();
    let built = Cell::new(0);
    let mut replayed = 0;
    for frame in 0..3i64 {
        replayed = 0;
        let mut ui = core.frame(VIEW, 1.0);
        ui.configure_root(NodeSpec::column().fill());
        for i in 0..5 {
            let line = frame + 4 - i;
            let a = ui.memo_indexed(line as u64, &Value::Int(line), |ui| {
                built.set(built.get() + 1);
                ui.text(&format!("line {line}"), TextStyle::new(12.0));
            });
            replayed += usize::from(a.replayed());
        }
        ui.finish();
    }
    assert_eq!(replayed, 4, "every line but the new one");
    assert_eq!(built.get(), 5 + 1 + 1);
}

#[test]
fn a_memo_inside_a_component_is_kept_with_it_and_on_its_own() {
    let mut core = new_core();
    let views = [Cell::new(0), Cell::new(0)];
    let run = |core: &mut Core, title: &str| {
        let mut ui = core.frame(VIEW, 1.0);
        ui.configure_root(NodeSpec::column().fill());
        let mut inner = None;
        let props = Value::map([("title", Value::str(title))]);
        let outer = ui.component_by(
            "test/pane",
            &props,
            |_, _| {},
            |_, ui| {
                views[0].set(views[0].get() + 1);
                ui.text(title, TextStyle::new(14.0));
                inner = Some(ui.memo("body", &Value::Null, |ui| {
                    views[1].set(views[1].get() + 1);
                    for i in 0..3 {
                        ui.text(&format!("line {i}"), TextStyle::new(12.0));
                    }
                }));
            },
        );
        ui.finish();
        (outer, inner)
    };
    use FillAnswer::*;
    assert_eq!(run(&mut core, "a"), (Some(NotKept), Some(NotKept)));
    let fresh = snapshot(&mut core);
    assert_eq!(run(&mut core, "a"), (Some(Replayed), None));
    assert_eq!(snapshot(&mut core), fresh);
    // The component's props moved: it fills again, the memo replays in it.
    assert_eq!(run(&mut core, "b"), (Some(Params), Some(Replayed)));
    assert_eq!((views[0].get(), views[1].get()), (2, 1));
    assert_eq!(run(&mut core, "b"), (Some(Replayed), None));
    assert_eq!(
        core.access_tree()
            .nodes
            .iter()
            .filter(|n| n.name.as_deref().is_some_and(|t| t.starts_with("line")))
            .count(),
        3
    );
}

#[test]
fn a_component_inside_a_memo_is_placed_again_by_its_replay() {
    let mut core = new_core();
    let views = [Cell::new(0), Cell::new(0)];
    let run = |core: &mut Core| {
        let mut ui = core.frame(VIEW, 1.0);
        ui.configure_root(NodeSpec::column().fill());
        let m = ui.memo("m", &Value::Null, |ui| {
            views[0].set(views[0].get() + 1);
            ui.text("before", TextStyle::new(12.0));
            ui.component_by(
                "test/inner",
                &Value::Null,
                |_, _| {},
                |_, ui| {
                    views[1].set(views[1].get() + 1);
                    ui.text("inside", TextStyle::new(12.0));
                },
            );
        });
        ui.finish();
        m
    };
    run(&mut core);
    let fresh = snapshot(&mut core);
    // A component a memo placed is journaled by name, and a closure fill
    // has no name to be placed by: the memo is not replayed.
    assert_eq!(run(&mut core), FillAnswer::NotReplayable);
    assert_eq!(snapshot(&mut core), fresh);
}
