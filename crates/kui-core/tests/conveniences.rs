//! The Rust view's shorthands from the DX sweep of 2026-09-27 (backlog
//! DX1–DX6): each is pinned to the long form it stands for, so a
//! shorthand that drifts from what it abbreviates fails here.

use kui_core::{
    Align, Core, FloatConfig, InputEvent, KeyMods, Mods, NodeSpec, QuadKind, Rect, Size, Sizing,
    TextStyle, UiEvent, Value, Vec2, widgets,
};

const VIEW: Size = Size { w: 300.0, h: 200.0 };

fn quads(core: &mut Core) -> Vec<QuadKind> {
    core.output().0.quads.iter().map(|q| q.kind).collect()
}

#[test]
fn a_number_is_fixed_px_and_grow_is_an_equal_share() {
    let long = NodeSpec::row()
        .width(Sizing::Fixed(20.0))
        .height(Sizing::Fixed(30.0));
    assert_eq!(NodeSpec::row().width(20.0).height(30.0), long);
    assert_eq!(NodeSpec::row().size(20.0, 30.0), long);
    assert_eq!(
        NodeSpec::row().grow_width().grow_height(),
        NodeSpec::row()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Grow(1.0))
    );
    assert_eq!(
        NodeSpec::row().fill(),
        NodeSpec::row().size(Sizing::GROW, Sizing::GROW)
    );
}

#[test]
fn value_takes_the_numbers_a_view_holds_and_reads_back_by_key() {
    assert_eq!(Value::from(3i32), Value::Int(3));
    assert_eq!(Value::from(3u32), Value::Int(3));
    assert_eq!(Value::from(3usize), Value::Int(3));
    assert_eq!(Value::from(usize::MAX), Value::Int(i64::MAX), "saturates");
    assert_eq!(Value::from(0.5f32), Value::Float(0.5));
    let v = Value::map([
        ("kind", "drag".into()),
        ("x", Value::from(1.5f32)),
        ("n", Value::from(4usize)),
        ("on", true.into()),
    ]);
    assert_eq!(v.get_str("kind"), Some("drag"));
    assert_eq!(v.get_f32("x"), Some(1.5));
    assert_eq!(v.get_float("n"), Some(4.0), "an int reads as a float");
    assert_eq!(v.get_int("n"), Some(4));
    assert_eq!(v.get_bool("on"), Some(true));
    assert_eq!(v.get_str("x"), None, "the wrong type is None");
    let ev = UiEvent::on(kui_core::OriginId::HOST, kui_core::Key::ROOT, v);
    assert_eq!(ev.kind(), Some("drag"));
    let plain = UiEvent::on(kui_core::OriginId::HOST, kui_core::Key::ROOT, "save".into());
    assert_eq!(plain.kind(), None, "a tag that is not a map has no kind");
}

#[test]
fn modal_takes_what_every_other_tag_takes_and_key_sink_is_a_null_tag() {
    assert_eq!(
        NodeSpec::column().modal("menu"),
        NodeSpec::column().modal(Value::str("menu"))
    );
    assert_eq!(
        NodeSpec::column().key_sink(),
        NodeSpec::column().on_key(Value::Null)
    );
}

#[test]
fn inside_is_the_same_point_on_the_anchor_and_the_float() {
    let (x, y) = (Align::End, Align::Start);
    let a = FloatConfig::viewport().inside(x, y);
    let b = FloatConfig::viewport().at(x, y).self_at(x, y);
    assert_eq!(
        (a.anchor_point, a.self_point),
        (b.anchor_point, b.self_point)
    );
}

#[test]
fn modifier_steps_match_the_literals() {
    assert_eq!(
        KeyMods::NONE.with_shift().with_ctrl(),
        KeyMods {
            shift: true,
            ctrl: true,
            ..Default::default()
        }
    );
    assert_eq!(KeyMods::NONE, KeyMods::default());
    assert!(KeyMods::NONE.with_primary().primary());
    assert_eq!(
        Mods::NONE.with_shift().with_word(),
        Mods {
            shift: true,
            word: true,
            doc: false
        }
    );
    assert_eq!(
        Rect::new(10.0, 20.0, 40.0, 60.0).center(),
        Vec2::new(30.0, 50.0)
    );
}

#[test]
fn text_in_is_a_box_around_a_text() {
    let build = |core: &mut Core, short: bool| {
        let mut ui = core.frame(VIEW, 1.0);
        let spec = NodeSpec::row().pad(4.0).bg(kui_core::Color::WHITE);
        let style = TextStyle::new(12.0);
        let key = if short {
            ui.text_in_keyed("t", spec, "hello", style)
        } else {
            ui.with_keyed("t", spec, |ui| ui.text("hello", style))
        };
        ui.finish();
        (key, format!("{:?}", core.output().0.quads))
    };
    let (mut a, mut b) = (Core::new(), Core::new());
    assert_eq!(build(&mut a, true), build(&mut b, false));
}

/// `NodeSpec::tooltip` is the prop whole: before it, a Rust view's
/// `apply_tooltip` made the node hoverable and described and floated
/// nothing, so a hint the app wrote was never seen (from kawoosh's
/// settings pane, 2026-09-27).
#[test]
fn a_spec_tooltip_floats_while_hovered_and_apply_tooltip_alone_does_not() {
    let frame = |core: &mut Core, spec: NodeSpec| {
        let mut ui = core.frame(VIEW, 1.0);
        ui.leaf_keyed("tip", spec.size(100.0, 40.0));
        ui.finish();
    };
    let mut core = Core::new();
    frame(&mut core, NodeSpec::column().tooltip("a hint"));
    assert!(
        quads(&mut core).is_empty(),
        "nothing hovered, nothing floats"
    );

    core.handle_input(InputEvent::CursorMoved(Vec2::new(10.0, 10.0)));
    frame(&mut core, NodeSpec::column().tooltip("a hint"));
    let q = quads(&mut core);
    assert!(
        q.contains(&QuadKind::Solid) && q.iter().any(|k| matches!(k, QuadKind::GlyphMask)),
        "hovered: the hint's box and glyphs, {q:?}"
    );

    frame(&mut core, NodeSpec::column().apply_tooltip("a hint"));
    assert!(
        quads(&mut core).is_empty(),
        "the spec half alone floats nothing, as a binding that floats its own needs"
    );

    core.handle_input(InputEvent::CursorMoved(Vec2::new(250.0, 150.0)));
    frame(&mut core, NodeSpec::column().tooltip("a hint"));
    assert!(quads(&mut core).is_empty(), "unhovered: the hint is gone");
}

#[test]
fn a_button_given_a_hint_and_a_spec_tooltip_floats_one_hint() {
    let frame = |core: &mut Core, spec: NodeSpec| {
        let mut ui = core.frame(VIEW, 1.0);
        widgets::button_with(&mut ui, "b", "go", spec, Some("a hint"));
        ui.finish();
        core.output().0.quads.len()
    };
    let hovered = || {
        let mut core = Core::new();
        let spec = NodeSpec::row();
        frame(&mut core, spec);
        core.handle_input(InputEvent::CursorMoved(Vec2::new(5.0, 5.0)));
        core
    };
    let bare = frame(&mut Core::new(), NodeSpec::row());
    let one = frame(&mut hovered(), NodeSpec::row());
    assert!(one > bare, "hovered, the hint floats: {one} vs {bare}");
    let both = frame(&mut hovered(), NodeSpec::row().tooltip("a hint"));
    assert_eq!(both, one, "the widget's hint, and not the spec's as well");
}

/// DX8: a label and an index as one key, with no string made. Before,
/// kawoosh formatted `gap{id}` (twice, once for `child_key` and once for
/// `with_keyed`) or offset its indexes (`2000 + i`) to stay clear of the
/// sibling-index keys.
#[test]
fn a_built_key_opens_as_built_and_clashes_with_nothing() {
    let mut core = Core::new();
    let mut ui = core.frame(VIEW, 1.0);
    let row = ui.with_keyed("row", NodeSpec::row(), |ui| {
        let first = ui.leaf_indexed(0, NodeSpec::row().size(10.0, 10.0));
        let gaps: Vec<_> = (0..3u64)
            .map(|i| {
                let k = ui.child_key("gap").index(i);
                assert_eq!(ui.leaf_key(k, NodeSpec::row().size(10.0, 10.0)), k);
                k
            })
            .collect();
        let tabs = ui.child_key("tabs").index(0);
        ui.with_key(tabs, NodeSpec::row(), |ui| {
            ui.text("t", TextStyle::new(12.0))
        });
        assert!(!gaps.contains(&first) && !gaps.contains(&tabs));
        assert_eq!(
            gaps[1],
            ui.child_key("gap").index(1),
            "the same key each time"
        );
    });
    ui.finish();
    assert_eq!(row, kui_core::Key::ROOT.str("row"));
    let codes: Vec<_> = core.take_warnings().iter().map(|w| w.code).collect();
    assert!(!codes.contains(&"duplicate-key"), "{codes:?}");
    assert_eq!(core.key_of("gap"), None, "a built key has no label");

    // The same key twice in a frame is the warning two labels are.
    let mut ui = core.frame(VIEW, 1.0);
    let k = ui.child_key("gap").index(0);
    ui.leaf_key(k, NodeSpec::row());
    ui.leaf_key(k, NodeSpec::row());
    ui.finish();
    let codes: Vec<_> = core.take_warnings().iter().map(|w| w.code).collect();
    assert!(codes.contains(&"duplicate-key"), "{codes:?}");
}
