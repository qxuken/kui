//! An access rect is what is drawn (backlog F93): every node's rect, and
//! every run of an editor's text, is cut to the clip the node was emitted
//! under — the one its hit region carries. A node wholly clipped keeps its
//! place in the tree and its actions on a zero-size rect at the clip's
//! edge, so a reader's hover over whatever is drawn there never finds it.

use kui_core::{
    AccessAction, Core, EditOptions, FloatConfig, Key, NodeSpec, Rect, Role, Size, Sizing,
    TextStyle, Value, Vec2,
};

fn rect_of(core: &mut Core, key: Key) -> Rect {
    core.access_tree().get(key).expect("an access node").rect
}

fn button(w: f32, h: f32, name: &str) -> NodeSpec {
    NodeSpec::column()
        .width(Sizing::Fixed(w))
        .height(Sizing::Fixed(h))
        .on_click(Value::str(name))
        .label(name)
}

/// The mind map's shape (F90, F93): a 40 px toolbar over a `clip` canvas,
/// and one 80 × 40 node on the canvas at `(40, dy)` from its corner.
/// Returns the node's key.
fn canvas_with_a_node(core: &mut Core, float: FloatConfig, dy: f32) -> Key {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed(
        "toolbar",
        button(400.0, 40.0, "toolbar").width(Sizing::Grow(1.0)),
        |_| {},
    );
    let mut node = Key::ROOT;
    ui.with(NodeSpec::column().fill().clip(), |ui| {
        node = ui.with_keyed(
            "node",
            button(80.0, 40.0, "node").float(float.offset(40.0, dy)),
            |_| {},
        );
    });
    ui.finish();
    node
}

/// The report: a node panned 20 px under the toolbar is cut at the
/// canvas's edge, and its access rect with it — no longer over the
/// toolbar, where a reader's highlight used to be drawn.
#[test]
fn a_clipped_float_is_cut_at_its_parents_edge() {
    let mut core = Core::new();
    let node = canvas_with_a_node(&mut core, FloatConfig::parent().clipped(), -20.0);
    assert_eq!(rect_of(&mut core, node), Rect::new(40.0, 40.0, 80.0, 20.0));
}

/// Wholly past the edge, it keeps its place in reading order and its
/// actions, on a zero-size rect at the edge nearest it.
#[test]
fn a_node_wholly_clipped_is_a_point_on_the_clips_edge() {
    let mut core = Core::new();
    let node = canvas_with_a_node(&mut core, FloatConfig::parent().clipped(), -60.0);
    let tree = core.access_tree();
    let n = tree.get(node).expect("still in the tree");
    assert_eq!(n.rect, Rect::new(40.0, 40.0, 0.0, 0.0));
    assert_eq!(n.name.as_deref(), Some("node"));
    let actions = n.action_list();
    assert!(actions.contains(&AccessAction::Click), "{actions:?}");
    assert!(
        actions.contains(&AccessAction::ScrollIntoView),
        "{actions:?}"
    );
}

/// A float that escapes its parent's clip escapes it here too: its rect
/// is its whole box, over the toolbar, where it draws and takes presses.
#[test]
fn an_escaping_float_keeps_its_whole_box() {
    let mut core = Core::new();
    let node = canvas_with_a_node(&mut core, FloatConfig::parent(), -20.0);
    assert_eq!(rect_of(&mut core, node), Rect::new(40.0, 20.0, 80.0, 40.0));
}

/// In flow, a child wider than a plain `clip` box is cut at its side. The
/// box itself is elided from the tree, which is why the clip is applied
/// in the core rather than left to the platform's `clips_children`.
#[test]
fn a_child_of_a_clip_box_is_cut_at_its_side() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let mut wide = Key::ROOT;
    ui.with(
        NodeSpec::column()
            .width(Sizing::Fixed(100.0))
            .height(Sizing::Fixed(50.0))
            .clip(),
        |ui| wide = ui.with_keyed("wide", button(160.0, 30.0, "wide"), |_| {}),
    );
    ui.finish();
    assert_eq!(rect_of(&mut core, wide), Rect::new(0.0, 0.0, 100.0, 30.0));
}

/// A 50 px scroller over rows 30 px tall. Returns the scroller and the
/// three rows' keys.
fn scroller(core: &mut Core) -> (Key, [Key; 3]) {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().pad(10.0));
    let mut rows = [Key::ROOT; 3];
    let list = ui.with_keyed(
        "list",
        NodeSpec::column()
            .width(Sizing::Fixed(120.0))
            .height(Sizing::Fixed(50.0))
            .scroll_y(),
        |ui| {
            for (i, row) in rows.iter_mut().enumerate() {
                let name = format!("row {i}");
                *row = ui.with_keyed(&name, button(120.0, 30.0, &name), |_| {});
            }
        },
    );
    ui.finish();
    (list, rows)
}

/// A row half out of a scroller is cut at its edge, and a row wholly out
/// is a point on it; scrolled, the same holds at the other edge.
#[test]
fn a_row_half_out_of_a_scroller_is_cut_at_its_edge() {
    let mut core = Core::new();
    let (list, rows) = scroller(&mut core);
    assert_eq!(
        core.access_tree().get(list).unwrap().rect,
        Rect::new(10.0, 10.0, 120.0, 50.0),
        "the scroller is not cut by itself"
    );
    assert_eq!(
        rect_of(&mut core, rows[0]),
        Rect::new(10.0, 10.0, 120.0, 30.0)
    );
    assert_eq!(
        rect_of(&mut core, rows[1]),
        Rect::new(10.0, 40.0, 120.0, 20.0)
    );
    assert_eq!(rect_of(&mut core, rows[2]), Rect::new(10.0, 60.0, 0.0, 0.0));

    // Scrolled 40 px: the first row is above the top edge, the second
    // straddles it, the third is inside.
    core.set_scroll(list, Vec2::new(0.0, 40.0));
    scroller(&mut core);
    assert_eq!(rect_of(&mut core, rows[0]), Rect::new(10.0, 10.0, 0.0, 0.0));
    assert_eq!(
        rect_of(&mut core, rows[1]),
        Rect::new(10.0, 10.0, 120.0, 20.0)
    );
    assert_eq!(
        rect_of(&mut core, rows[2]),
        Rect::new(10.0, 30.0, 120.0, 30.0)
    );
}

/// The cache cannot keep a rect the clip moved: the node's box is the
/// same in both frames, only its clipping parent narrowed.
#[test]
fn a_clip_that_moved_moves_the_tree() {
    let mut core = Core::new();
    let mut draw = |w: f32| {
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        let mut key = Key::ROOT;
        ui.with(
            NodeSpec::column()
                .width(Sizing::Fixed(w))
                .height(Sizing::Fixed(50.0))
                .clip(),
            |ui| key = ui.with_keyed("b", button(160.0, 30.0, "b"), |_| {}),
        );
        ui.finish();
        core.access_tree().get(key).unwrap().rect
    };
    assert_eq!(draw(100.0).w, 100.0);
    assert_eq!(draw(60.0).w, 60.0);
}

/// A document in a horizontal scroller, scrolled 30 px: its run is cut at
/// the scroller's left edge and every character stays where it is drawn,
/// its position now relative to the cut run's x.
#[test]
fn an_editors_runs_are_cut_and_their_characters_stay_put() {
    let draw = |core: &mut Core| {
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        let mut edit = Key::ROOT;
        let list = ui.with_keyed(
            "h",
            NodeSpec::column()
                .width(Sizing::Fixed(100.0))
                .height(Sizing::Fixed(60.0))
                .scroll_x(),
            |ui| {
                edit = ui.text_edit(
                    "doc",
                    "hello world, a line wider than the scroller",
                    &EditOptions {
                        multiline: true,
                        ..Default::default()
                    },
                    NodeSpec::column().width(Sizing::Fixed(400.0)).label("Doc"),
                );
            },
        );
        ui.finish();
        (list, edit)
    };
    let mut core = Core::new();
    let (list, edit) = draw(&mut core);
    let before = core.access_tree().get(edit).unwrap().runs[0].clone();
    assert_eq!(before.rect.x, 0.0);
    assert!(
        before.rect.w <= 100.0,
        "cut at the right edge: {:?}",
        before.rect
    );

    core.set_scroll(list, Vec2::new(30.0, 0.0));
    draw(&mut core);
    let after = core.access_tree().get(edit).unwrap().runs[0].clone();
    assert_eq!(after.rect.x, 0.0, "cut at the left edge");
    for (b, a) in before.char_positions.iter().zip(&after.char_positions) {
        assert_eq!(
            after.rect.x + a,
            before.rect.x + b - 30.0,
            "a character moved by the scroll and nothing else"
        );
    }
}

/// A custom editor's lines are cut by the clip their own nodes were drawn
/// under: in a scroller one line tall, the second line is a point on its
/// bottom edge.
#[test]
fn a_custom_editors_runs_are_cut_by_their_lines_clip() {
    let mut core = Core::new();
    let style = TextStyle::new(14.0);
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let sink = ui.with_keyed(
        "doc",
        NodeSpec::column()
            .on_key(Value::str("doc"))
            .role(Role::MultilineTextInput)
            .label("Doc"),
        |ui| {
            ui.with(
                NodeSpec::column()
                    .width(Sizing::Fixed(200.0))
                    .height(Sizing::Fixed(20.0))
                    .scroll_y(),
                |ui| {
                    for i in 0..2 {
                        ui.with_keyed(
                            &format!("l{i}"),
                            NodeSpec::row().role(Role::Line).height(Sizing::Fixed(20.0)),
                            |ui| ui.text(&format!("line {i}"), style),
                        );
                    }
                },
            );
        },
    );
    ui.finish();
    let node = core.access_tree().get(sink).unwrap().clone();
    assert_eq!(node.value.as_deref(), Some("line 0\nline 1"));
    assert_eq!(node.runs.len(), 2);
    assert!(node.runs[0].rect.h > 0.0 && node.runs[0].rect.w > 0.0);
    assert!(node.runs[0].rect.y + node.runs[0].rect.h <= 20.0);
    assert_eq!(
        (
            node.runs[1].rect.y,
            node.runs[1].rect.w,
            node.runs[1].rect.h
        ),
        (20.0, 0.0, 0.0)
    );
}
