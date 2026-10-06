//! A leaf's `tooltip` (backlog RG113): a `line`, `polygon`, `path` or
//! `cells` grid holds no children, so its hint cannot float as its last
//! child the way a box's does. It floats beside the leaf instead, anchored
//! to it by key, and lands below the leaf's box — drawn only while the
//! leaf is hovered, hit by its shape, out of every clip and out of the
//! layout its siblings see.

use kui_core::cells::{Cell, CellGrid};
use kui_core::schema::PropsOut;
use kui_core::testing::hover;
use kui_core::{
    Color, Content, Core, FillRule, Key, NodeSpec, QuadKind, Rect, Size, Stroke, TextStyle, Vec2,
};

const VIEW: Size = Size { w: 400.0, h: 300.0 };

/// A prop list that keys its node `label` and declares a `tooltip`, the
/// way every binding's parser leaves one. `on_layout`, so `layout_of`
/// reads the leaf's box back.
fn tipped(label: &str, spec: NodeSpec, hint: &str) -> PropsOut {
    let mut p = PropsOut::new();
    p.spec = spec.on_layout("laid");
    p.key = Some(label.into());
    p.apply_tooltip(hint);
    p
}

/// The hint's chrome on the wire, and the rect of the clip it is drawn
/// in: the quad in the theme's `raised`, the tooltip surface's background.
fn hint_quad(core: &mut Core) -> Option<(Rect, Rect)> {
    let raised = core.theme().raised;
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .find(|q| q.kind == QuadKind::Solid && q.color == raised)
        .map(|q| (q.rect, dl.clips[q.clip as usize].rect))
}

/// Whether any glyph is on the wire — the hint's text, in these scenes.
fn has_glyphs(core: &mut Core) -> bool {
    core.output().0.quads.iter().any(|q| {
        matches!(
            q.kind,
            QuadKind::GlyphColor | QuadKind::GlyphSubpixel | QuadKind::GlyphMask
        )
    })
}

/// The hint hangs below the leaf's box, centred on it, with the tooltip
/// preset's 6px gap — what it is for a box. Returns the hint's rect and
/// its clip's.
fn assert_below(core: &mut Core, leaf: Key) -> (Rect, Rect) {
    let l = core.layout_of(leaf).expect("the leaf is laid out");
    let (t, clip) = hint_quad(core).expect("the hint floats");
    assert_eq!(t.y, l.y + l.h + 6.0, "below the leaf's box: {l:?} {t:?}");
    assert!(
        ((t.x + t.w / 2.0) - (l.x + l.w / 2.0)).abs() < 0.01,
        "centred on the leaf's box: {l:?} {t:?}"
    );
    assert!(t.w > 0.0 && t.h > 0.0, "sized to its text: {t:?}");
    (t, clip)
}

/// A pie's wedge through `open_from`, the door JSX and Lua lower it by:
/// nothing floats until the pointer is inside its outline, the hint then
/// sits below its box, and a pointer in the box past the outline is not
/// over the wedge.
#[test]
fn a_hovered_wedge_floats_its_tooltip_below_its_box() {
    let mut core = Core::new();
    let pts = [
        Vec2::new(100.0, 50.0),
        Vec2::new(200.0, 50.0),
        Vec2::new(150.0, 150.0),
    ];
    let frame = |core: &mut Core| {
        let mut ui = core.frame(VIEW, 1.0);
        let p = tipped("wedge", NodeSpec::column().bg(Color::WHITE), "a wedge");
        let key = ui.core().open_from(p, Content::Polygon(&pts));
        ui.finish();
        key
    };
    let key = frame(&mut core);
    assert_eq!(hint_quad(&mut core), None, "not hovered: nothing floats");
    assert!(!has_glyphs(&mut core));

    hover(&mut core, Vec2::new(150.0, 70.0));
    frame(&mut core);
    assert_below(&mut core, key);
    assert!(has_glyphs(&mut core), "and its text");

    // In the wedge's box, past its outline: the wedge is hit by its shape,
    // so it is not hovered and its hint is gone.
    hover(&mut core, Vec2::new(105.0, 140.0));
    frame(&mut core);
    assert_eq!(hint_quad(&mut core), None, "outside the outline");
    assert!(!has_glyphs(&mut core));
}

/// A path in a clipped box, through the `d` door: its hint lands below the
/// path's box past the clip's edge and is drawn there, since a tooltip
/// escapes its ancestors' clips as a box's does.
#[test]
fn a_paths_tooltip_escapes_the_clip_it_hangs_from() {
    let mut core = Core::new();
    let frame = |core: &mut Core| {
        let mut ui = core.frame(VIEW, 1.0);
        ui.open(NodeSpec::column().size(200.0, 95.0).clip());
        let p = tipped("arrow", NodeSpec::column().bg(Color::WHITE), "an arrow");
        let key = ui.core().open_from(
            p,
            Content::PathD("M 20 20 L 120 20 L 70 90 Z", FillRule::NonZero, None, None),
        );
        ui.close();
        ui.finish();
        key
    };
    let key = frame(&mut core);
    assert_eq!(hint_quad(&mut core), None, "not hovered: nothing floats");

    hover(&mut core, Vec2::new(70.0, 30.0));
    frame(&mut core);
    let (rect, clip) = assert_below(&mut core, key);
    assert!(rect.y >= 95.0, "below the clipping box's edge: {rect:?}");
    assert!(
        clip.y + clip.h >= rect.y + rect.h,
        "and not cut by it: {rect:?} in {clip:?}"
    );
}

/// The Rust door: a `line` whose spec says `NodeSpec::tooltip`, hovered
/// along its stroke. And the hint is out of the layout and out of the
/// sibling slots: the box declared after the line keeps its place and its
/// auto key whether the hint is there or not.
#[test]
fn a_lines_tooltip_floats_and_its_siblings_keep_their_keys() {
    let mut core = Core::new();
    let frame = |core: &mut Core| {
        let mut ui = core.frame(VIEW, 1.0);
        ui.line_keyed(
            "link",
            Vec2::new(40.0, 40.0),
            Vec2::new(240.0, 40.0),
            Stroke::new(2.0, Color::WHITE),
            NodeSpec::column().tooltip("a link").on_layout("laid"),
        );
        let after = ui.leaf(NodeSpec::column().size(50.0, 20.0).on_layout("laid"));
        ui.finish();
        after
    };
    let after = frame(&mut core);
    let line = Key::ROOT.str("link");
    let placed = core.layout_of(after);
    assert!(placed.is_some());
    assert_eq!(hint_quad(&mut core), None, "not hovered: nothing floats");

    hover(&mut core, Vec2::new(140.0, 40.0));
    assert_eq!(frame(&mut core), after, "the next sibling's key is unmoved");
    assert_below(&mut core, line);
    assert_eq!(core.layout_of(after), placed, "and so is its place");

    hover(&mut core, Vec2::new(140.0, 200.0));
    frame(&mut core);
    assert_eq!(hint_quad(&mut core), None, "unhovered: gone again");
}

/// A `cells` grid through `open_from` — the one leaf of the four that is
/// in flow — floats its hint below it too.
#[test]
fn a_cells_grids_tooltip_floats_below_it() {
    let mut core = Core::new();
    let cells = vec![Cell::default(); 8 * 2];
    let frame = |core: &mut Core| {
        let mut ui = core.frame(VIEW, 1.0);
        let grid = CellGrid {
            rows: 2,
            cols: 8,
            cells: &cells,
            style: TextStyle::new(14.0),
            cursor: None,
            origin_line: 0,
        };
        let p = tipped("term", NodeSpec::column(), "a terminal");
        let key = ui.core().open_from(p, Content::Cells(&grid));
        ui.finish();
        key
    };
    let key = frame(&mut core);
    assert_eq!(hint_quad(&mut core), None, "not hovered: nothing floats");
    hover(&mut core, Vec2::new(5.0, 5.0));
    frame(&mut core);
    assert_below(&mut core, key);
}

/// A leaf near the window's bottom: the hint flips above it, as a box's
/// does under `fit` — the node-anchored placement keeps the tooltip
/// preset's `fit` rather than dropping it.
#[test]
fn a_leafs_tooltip_flips_above_near_the_bottom() {
    let mut core = Core::new();
    let pts = [
        Vec2::new(100.0, 250.0),
        Vec2::new(200.0, 250.0),
        Vec2::new(150.0, 295.0),
    ];
    let frame = |core: &mut Core| {
        let mut ui = core.frame(VIEW, 1.0);
        let p = tipped("low", NodeSpec::column().bg(Color::WHITE), "low");
        let key = ui.core().open_from(p, Content::Polygon(&pts));
        ui.finish();
        key
    };
    let key = frame(&mut core);
    hover(&mut core, Vec2::new(150.0, 260.0));
    frame(&mut core);
    let l = core.layout_of(key).unwrap();
    let (t, _) = hint_quad(&mut core).expect("the hint floats");
    assert_eq!(t.y + t.h, l.y - 6.0, "above the leaf's box: {l:?} {t:?}");
}
