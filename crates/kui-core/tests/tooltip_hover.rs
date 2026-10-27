//! When a box's `tooltip` shows (backlog F179): while the pointer is over
//! the box or anything inside it, as a browser's `title` does. A child
//! that tracks the pointer — a `hover_bg`, an `on_click`, a button — is
//! the hovered node over its own box, and the hint used to show only over
//! the box's bare background. With hints nested, the innermost one under
//! the pointer is the one shown.

use kui_core::schema::PropsOut;
use kui_core::testing::hover;
use kui_core::{Color, Content, Core, FloatConfig, NodeSpec, QuadKind, Rect, Size, Vec2, widgets};

/// A view a case builds.
type Child = dyn Fn(&mut kui_core::Ui<'_>);

const VIEW: Size = Size { w: 400.0, h: 300.0 };

/// Every hint's background on the wire: the solid quads in the theme's
/// `raised`, the tooltip surface's colour.
fn hints(core: &mut Core) -> Vec<Rect> {
    let raised = core.theme().raised;
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .filter(|q| q.kind == QuadKind::Solid && q.color == raised)
        .map(|q| q.rect)
        .collect()
}

/// A 100×60 box padded 10 with a `tooltip`, around a 40×20 child built by
/// `child`: the child at (10, 10)–(50, 30), the box's bare background
/// below and right of it.
fn frame(core: &mut Core, child: &dyn Fn(&mut kui_core::Ui<'_>)) {
    let mut ui = core.frame(VIEW, 1.0);
    ui.with(
        NodeSpec::column()
            .size(100.0, 60.0)
            .pad(10.0)
            .tooltip("outer"),
        |ui| child(ui),
    );
    ui.finish();
}

/// Moves the pointer to `at` and builds the frame under it, twice: the
/// first frame lays out what the move found, the second is what a host
/// draws next.
fn at(core: &mut Core, p: Vec2, child: &dyn Fn(&mut kui_core::Ui<'_>)) -> Vec<Rect> {
    hover(core, p);
    frame(core, child);
    frame(core, child);
    hints(core)
}

/// A child with a `hover_bg` and one with an `on_click` each take the
/// hover over themselves; the box's hint shows over them as it does over
/// the box's background, and not outside the box.
#[test]
fn a_boxs_tooltip_shows_over_a_child_that_tracks_the_pointer() {
    let kids: [(&str, NodeSpec); 2] = [
        (
            "hover_bg",
            NodeSpec::column()
                .size(40.0, 20.0)
                .hover_bg(Color::hex(0x00ff00ff)),
        ),
        (
            "on_click",
            NodeSpec::column().size(40.0, 20.0).on_click("x"),
        ),
    ];
    for (name, spec) in kids {
        let child = move |ui: &mut kui_core::Ui<'_>| {
            ui.with(spec.clone(), |_| {});
        };
        let mut core = Core::new();
        frame(&mut core, &child);
        let over_child = at(&mut core, Vec2::new(20.0, 20.0), &child);
        assert_eq!(
            over_child.len(),
            1,
            "{name}: over the child: {over_child:?}"
        );
        assert_eq!(
            over_child[0].y, 66.0,
            "{name}: below the box: {over_child:?}"
        );
        let over_box = at(&mut core, Vec2::new(80.0, 50.0), &child);
        assert_eq!(over_box.len(), 1, "{name}: over the box: {over_box:?}");
        let outside = at(&mut core, Vec2::new(200.0, 200.0), &child);
        assert!(outside.is_empty(), "{name}: outside: {outside:?}");
    }
}

/// A child with a `tooltip` of its own: over it, its hint and not the
/// box's — the innermost wins; over the box's background, the box's.
#[test]
fn the_innermost_tooltip_under_the_pointer_is_the_one_shown() {
    let child = |ui: &mut kui_core::Ui<'_>| {
        ui.with(NodeSpec::column().size(40.0, 20.0).tooltip("inner"), |_| {});
    };
    let mut core = Core::new();
    frame(&mut core, &child);
    let over_child = at(&mut core, Vec2::new(20.0, 20.0), &child);
    assert_eq!(over_child.len(), 1, "one hint: {over_child:?}");
    assert_eq!(
        over_child[0].y, 36.0,
        "the child's, below it: {over_child:?}"
    );
    let over_box = at(&mut core, Vec2::new(80.0, 50.0), &child);
    assert_eq!(over_box.len(), 1, "one hint: {over_box:?}");
    assert_eq!(over_box[0].y, 66.0, "the box's, below it: {over_box:?}");
}

/// A leaf with a `tooltip` inside the box — a polygon, whose hint floats
/// beside it rather than in it — wins over the box's as a child box's
/// does. Its points are in the box's space, so it covers (0, 0)–(40, 20)
/// and its hint hangs just below that, well above the box's.
#[test]
fn a_leafs_tooltip_inside_a_box_wins_over_the_boxs() {
    let pts = [
        Vec2::new(0.0, 0.0),
        Vec2::new(40.0, 0.0),
        Vec2::new(40.0, 20.0),
        Vec2::new(0.0, 20.0),
    ];
    let child = move |ui: &mut kui_core::Ui<'_>| {
        let mut p = PropsOut::new();
        p.spec = NodeSpec::column().size(40.0, 20.0).bg(Color::WHITE);
        p.key = Some("leaf".into());
        p.apply_tooltip("leaf");
        ui.core().open_from(p, Content::Polygon(&pts));
    };
    let mut core = Core::new();
    frame(&mut core, &child);
    let over_leaf = at(&mut core, Vec2::new(20.0, 10.0), &child);
    assert_eq!(over_leaf.len(), 1, "one hint: {over_leaf:?}");
    let y = over_leaf[0].y;
    assert!(
        (26.0..30.0).contains(&y),
        "the leaf's, below it: {over_leaf:?}"
    );
    let over_box = at(&mut core, Vec2::new(80.0, 50.0), &child);
    assert_eq!(over_box.len(), 1, "one hint: {over_box:?}");
    assert_eq!(over_box[0].y, 66.0, "the box's, below it: {over_box:?}");
}

/// A stock button, checkbox or slider with a hint of its own floats that
/// hint from inside its body, not through the prop; over it, its hint
/// and not the box's as well (backlog RG159: the box's floated too, since
/// only the prop's path said a hint had).
#[test]
fn a_stock_controls_own_hint_is_the_innermost() {
    let kids: [(&str, &Child); 3] = [
        ("button", &|ui| {
            let spec = widgets::button_spec(&ui.theme(), &ui.metrics()).on_click("go");
            widgets::button_with(ui, "go", "Go", spec, Some("inner"));
        }),
        ("checkbox", &|ui| {
            let spec = NodeSpec::row().on_click("tick");
            widgets::toggle_with(
                ui,
                widgets::Toggle::Checkbox,
                "tick",
                "Tick",
                spec,
                Some("inner"),
            );
        }),
        ("slider", &|ui| {
            let spec = widgets::slider_spec(&ui.metrics())
                .width(60.0)
                .value_now(0.5)
                .value_min(0.0)
                .value_max(1.0)
                .on_change("v");
            widgets::slider_with(ui, "v", spec, Some("inner"));
        }),
    ];
    for (name, child) in kids {
        let mut core = Core::new();
        frame(&mut core, child);
        let over_child = at(&mut core, Vec2::new(14.0, 14.0), child);
        assert_eq!(over_child.len(), 1, "{name}: one hint: {over_child:?}");
        assert_ne!(
            over_child[0].y, 66.0,
            "{name}: the control's, not the box's: {over_child:?}"
        );
    }
}

/// A dialog declared inside a tooltip box — a viewport float — is placed
/// somewhere else: over it, or over a button in it, the box's hint stays
/// down. A badge floated against the box is still inside (backlog
/// RG160).
#[test]
fn a_dialog_declared_inside_a_tooltip_box_is_not_inside_it() {
    let dialog = |ui: &mut kui_core::Ui<'_>| {
        ui.with(
            NodeSpec::column()
                .size(80.0, 40.0)
                .float(FloatConfig::viewport().offset(250.0, 150.0))
                .hover_bg(Color::hex(0x00ff00ff)),
            |ui| {
                ui.with(NodeSpec::column().size(20.0, 20.0).on_click("ok"), |_| {});
            },
        );
    };
    let mut core = Core::new();
    frame(&mut core, &dialog);
    let over_dialog = at(&mut core, Vec2::new(290.0, 180.0), &dialog);
    assert!(over_dialog.is_empty(), "over the dialog: {over_dialog:?}");
    let over_button = at(&mut core, Vec2::new(255.0, 155.0), &dialog);
    assert!(over_button.is_empty(), "over its button: {over_button:?}");
    let over_box = at(&mut core, Vec2::new(50.0, 30.0), &dialog);
    assert_eq!(over_box.len(), 1, "over the box: {over_box:?}");

    let badge = |ui: &mut kui_core::Ui<'_>| {
        ui.with(
            NodeSpec::column()
                .size(20.0, 20.0)
                .float(FloatConfig::parent().offset(10.0, 10.0))
                .hover_bg(Color::hex(0x00ff00ff)),
            |_| {},
        );
    };
    let mut core = Core::new();
    frame(&mut core, &badge);
    let over_badge = at(&mut core, Vec2::new(15.0, 15.0), &badge);
    assert_eq!(over_badge.len(), 1, "over a badge: {over_badge:?}");
}

/// Keyboard focus shows a node's hint as the pointer does: Tab onto a
/// focusable box with a `tooltip` and the hint floats, with the pointer
/// nowhere near; a click focuses without the hint coming from focus.
#[test]
fn a_tooltip_shows_while_the_keyboard_holds_focus() {
    let build = |core: &mut Core| {
        let mut ui = core.frame(VIEW, 1.0);
        ui.with(
            NodeSpec::column()
                .size(60.0, 20.0)
                .on_click("a")
                .label("first")
                .tooltip("the first"),
            |_| {},
        );
        ui.with(
            NodeSpec::column()
                .size(60.0, 20.0)
                .on_click("b")
                .label("second")
                .tooltip("the second"),
            |_| {},
        );
        ui.finish();
    };
    let mut core = Core::new();
    build(&mut core);
    build(&mut core);
    assert!(
        hints(&mut core).is_empty(),
        "nothing hovered, nothing focused"
    );
    kui_core::testing::tab(&mut core, false);
    build(&mut core);
    build(&mut core);
    assert_eq!(hints(&mut core).len(), 1, "the focused box's hint shows");
}
