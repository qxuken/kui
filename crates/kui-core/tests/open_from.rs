//! `Core::open_from`: the one door a binding lowers a parsed prop list
//! through. It owns the three things every binding used to re-derive per
//! element — which key the node gets, the keyboard-focus edge, and when
//! the `tooltip` hint floats — so they are pinned here once, against the
//! core, rather than once per binding.

use kui_core::schema::PropsOut;
use kui_core::{Content, Core, InputEvent, Key, NodeSpec, QuadKind, Size, Vec2};

const VIEW: Size = Size { w: 300.0, h: 200.0 };

fn props(spec: NodeSpec) -> PropsOut {
    let mut p = PropsOut::new();
    p.spec = spec.size(100.0, 40.0);
    p
}

#[test]
fn the_index_beats_the_label_and_both_are_remembered() {
    let mut core = Core::new();
    let mut ui = core.frame(VIEW, 1.0);
    let by_label = {
        let mut p = props(NodeSpec::column());
        p.key = Some("a".into());
        p
    };
    let by_index = {
        let mut p = props(NodeSpec::column());
        p.key = Some("ignored".into());
        p.index = Some(7);
        p
    };
    let ka = ui.core().open_from(by_label, Content::Box);
    ui.close();
    let ki = ui.core().open_from(by_index, Content::Box);
    ui.close();
    let auto = ui.core().open_from(props(NodeSpec::column()), Content::Box);
    ui.close();
    ui.finish();
    assert_eq!(ka, Key::ROOT.str("a"), "a label keys like `open_keyed`");
    assert_eq!(ki, Key::ROOT.index(7), "an index keys like `open_indexed`");
    // Only auto-keyed children consume a sibling slot, as with `open`.
    assert_eq!(auto, Key::ROOT.index(0), "the first auto-keyed child");
    assert_eq!(core.key_of("a"), Some(ka), "the label resolves by name");
    assert_eq!(
        core.key_of("ignored"),
        None,
        "the label an index beat was never declared"
    );
}

#[test]
fn key_focus_lands_on_the_node_and_a_tooltip_floats_only_while_hovered() {
    let mut core = Core::new();
    let frame = |core: &mut Core| {
        let mut ui = core.frame(VIEW, 1.0);
        let mut p = props(NodeSpec::column());
        p.key = Some("tip".into());
        p.key_focus = true;
        p.apply_tooltip("a hint");
        let key = ui.core().open_from(p, Content::Box);
        ui.close();
        ui.finish();
        key
    };
    let key = frame(&mut core);
    assert_eq!(core.key_focus(), Some(key), "keyFocus takes the keyboard");
    let quads = |core: &mut Core| core.output().0.quads.clone();
    assert_eq!(quads(&mut core).len(), 0, "no tooltip: nothing is hovered");

    core.handle_input(InputEvent::CursorMoved(Vec2::new(10.0, 10.0)));
    frame(&mut core);
    let q = quads(&mut core);
    assert!(
        q.iter().any(|q| q.kind == QuadKind::Solid)
            && q.iter().any(|q| matches!(
                q.kind,
                QuadKind::GlyphMask | QuadKind::GlyphColor | QuadKind::GlyphSubpixel
            )),
        "hovered: the hint's box and its glyphs float below the node, {:?}",
        q.iter().map(|q| q.kind).collect::<Vec<_>>()
    );

    core.handle_input(InputEvent::CursorMoved(Vec2::new(250.0, 150.0)));
    frame(&mut core);
    assert_eq!(quads(&mut core).len(), 0, "unhovered: the hint is gone");
}

#[test]
fn a_leaf_takes_its_identity_and_no_hint() {
    let mut core = Core::new();
    let mut ui = core.frame(VIEW, 1.0);
    let mut p = props(NodeSpec::column());
    p.key = Some("stroke".into());
    p.apply_tooltip("never shown");
    let pts = [Vec2::new(0.0, 0.0), Vec2::new(50.0, 50.0)];
    let key = ui.core().open_from(
        p,
        Content::Line(&pts, kui_core::Stroke::new(2.0, kui_core::Color::WHITE)),
    );
    ui.finish();
    assert_eq!(key, Key::ROOT.str("stroke"));
    assert_eq!(
        core.key_of("stroke"),
        Some(key),
        "a line's label resolves too"
    );
    let (dl, _) = core.output();
    assert!(
        dl.quads.iter().all(|q| q.kind == QuadKind::Segment),
        "the stroke, and nothing floated"
    );
}
