//! A hover hint's opacity (backlog F178): the hint the core floats under a
//! hovered box — the `tooltip` prop's — fades with the box's ancestors
//! and not with the box, as a leaf's hint, floated beside the leaf, always
//! did. A dimmed box keeps a hint that can be read; a fading panel still
//! takes the hints inside it with it.

use kui_core::schema::PropsOut;
use kui_core::testing::hover;
use kui_core::{Color, Content, Core, NodeSpec, QuadKind, Size, Vec2, widgets};

/// A view a case builds.
type Child = dyn Fn(&mut kui_core::Ui<'_>);

const VIEW: Size = Size { w: 400.0, h: 300.0 };

/// The hint's background on the wire, as a share of the theme's `raised`
/// alpha: 1 unfaded. A group opacity multiplies alpha only, so the quad
/// is the solid one in `raised`'s colour.
fn hint_alpha(core: &mut Core) -> Option<f32> {
    let raised = core.theme().raised;
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .find(|q| {
            q.kind == QuadKind::Solid
                && (q.color.r, q.color.g, q.color.b) == (raised.r, raised.g, raised.b)
        })
        .map(|q| q.color.a / raised.a)
}

fn close(a: Option<f32>, want: f32) -> bool {
    a.is_some_and(|a| (a - want).abs() < 1e-4)
}

/// A box at 0.4 with a `tooltip`, hovered: the box is dimmed, its hint is
/// not.
#[test]
fn a_dimmed_boxs_tooltip_is_not_dimmed() {
    let mut core = Core::new();
    let frame = |core: &mut Core| {
        let mut ui = core.frame(VIEW, 1.0);
        ui.with(
            NodeSpec::column()
                .size(100.0, 40.0)
                .bg(Color::hex(0xff0000ff))
                .opacity(0.4)
                .tooltip("a hint"),
            |_| {},
        );
        ui.finish();
    };
    frame(&mut core);
    assert_eq!(hint_alpha(&mut core), None, "not hovered: nothing floats");

    hover(&mut core, Vec2::new(50.0, 20.0));
    frame(&mut core);
    let a = hint_alpha(&mut core);
    assert!(close(a, 1.0), "the hint is not the box's 0.4: {a:?}");
    let red = core
        .output()
        .0
        .quads
        .iter()
        .find(|q| q.kind == QuadKind::Solid && q.color.r == 1.0 && q.color.g == 0.0)
        .map(|q| q.color.a);
    assert!(close(red, 0.4), "the box itself is: {red:?}");
}

/// The same box in a panel at 0.5: the hint takes the panel's 0.5, not
/// the box's 0.2 — a fading panel still fades the hints inside it.
#[test]
fn a_tooltip_fades_with_the_boxs_ancestors() {
    let mut core = Core::new();
    let frame = |core: &mut Core| {
        let mut ui = core.frame(VIEW, 1.0);
        ui.with(NodeSpec::column().opacity(0.5), |ui| {
            ui.with(
                NodeSpec::column()
                    .size(100.0, 40.0)
                    .opacity(0.4)
                    .tooltip("a hint"),
                |_| {},
            );
        });
        ui.finish();
    };
    frame(&mut core);
    hover(&mut core, Vec2::new(50.0, 20.0));
    frame(&mut core);
    let a = hint_alpha(&mut core);
    assert!(close(a, 0.5), "the panel's opacity, not the box's: {a:?}");
}

/// A leaf at 0.4 — a polygon through `open_from` — whose hint floats
/// beside it: unfaded, as a box's now is.
#[test]
fn a_dimmed_leafs_tooltip_is_not_dimmed_either() {
    let mut core = Core::new();
    let pts = [
        Vec2::new(0.0, 0.0),
        Vec2::new(100.0, 0.0),
        Vec2::new(100.0, 40.0),
        Vec2::new(0.0, 40.0),
    ];
    let frame = |core: &mut Core| {
        let mut ui = core.frame(VIEW, 1.0);
        let mut p = PropsOut::new();
        p.spec = NodeSpec::column().opacity(0.4);
        p.key = Some("leaf".into());
        p.apply_tooltip("a hint");
        ui.core().open_from(p, Content::Polygon(&pts));
        ui.finish();
    };
    frame(&mut core);
    hover(&mut core, Vec2::new(50.0, 20.0));
    frame(&mut core);
    let a = hint_alpha(&mut core);
    assert!(close(a, 1.0), "the leaf's hint is unfaded: {a:?}");
}

/// A `widgets::tooltip` the view places itself inside a dimmed box is the
/// box's content, and fades with it as content does.
#[test]
fn an_apps_own_tooltip_fades_with_its_box() {
    let mut core = Core::new();
    let mut ui = core.frame(VIEW, 1.0);
    ui.with(NodeSpec::column().size(100.0, 40.0).opacity(0.4), |ui| {
        widgets::tooltip(ui, "placed");
    });
    ui.finish();
    let a = hint_alpha(&mut core);
    assert!(close(a, 0.4), "the box's opacity: {a:?}");
}

/// A disabled checkbox and slider are dimmed by the theme's
/// `disabled_opacity` and keep their hover, so a hint can say why; the
/// hint is not dimmed with them, as a disabled button's is not (backlog
/// RG159: the toggles and the slider floated theirs as content).
#[test]
fn a_disabled_toggles_and_sliders_hint_is_not_dimmed() {
    let kids: [(&str, &Child); 2] = [
        ("checkbox", &|ui| {
            let spec = NodeSpec::row().on_click("tick").disabled(true);
            widgets::toggle_with(
                ui,
                widgets::Toggle::Checkbox,
                "tick",
                "Tick",
                spec,
                Some("why"),
            );
        }),
        ("slider", &|ui| {
            let spec = widgets::slider_spec(&ui.metrics())
                .width(60.0)
                .value_now(0.5)
                .value_min(0.0)
                .value_max(1.0)
                .on_change("v")
                .disabled(true);
            widgets::slider_with(ui, "v", spec, Some("why"));
        }),
    ];
    for (name, child) in kids {
        let mut core = Core::new();
        let frame = |core: &mut Core| {
            let mut ui = core.frame(VIEW, 1.0);
            child(&mut ui);
            ui.finish();
        };
        frame(&mut core);
        hover(&mut core, Vec2::new(4.0, 4.0));
        frame(&mut core);
        frame(&mut core);
        let a = hint_alpha(&mut core);
        assert!(close(a, 1.0), "{name}: the hint is not dimmed: {a:?}");
    }
}

/// No hint, or one drawn at 0: what a frame shows for a hint not drawn.
fn hidden(a: Option<f32>) -> bool {
    a.is_none_or(|a| a == 0.0)
}

/// A box at opacity 0 is not there to explain: hovered, its hint is not
/// drawn, where F178's rule — the opacity the box inherited — drew it in
/// full over nothing (backlog RG160).
#[test]
fn an_invisible_boxs_tooltip_is_not_drawn() {
    let mut core = Core::new();
    let frame = |core: &mut Core| {
        let mut ui = core.frame(VIEW, 1.0);
        ui.with(
            NodeSpec::column()
                .size(100.0, 40.0)
                .opacity(0.0)
                .tooltip("a hint"),
            |_| {},
        );
        ui.finish();
    };
    frame(&mut core);
    hover(&mut core, Vec2::new(50.0, 20.0));
    frame(&mut core);
    frame(&mut core);
    let a = hint_alpha(&mut core);
    assert!(hidden(a), "no hint over an invisible box: {a:?}");
}

/// A hovered box with a `tooltip` and an `exit`, removed: the ghost fades
/// out and its hint is not in the picture — the hover went with the box
/// (backlog RG160).
#[test]
fn a_departing_boxs_tooltip_leaves_with_the_hover() {
    let mut core = Core::new();
    core.set_time(0.0);
    let card = |core: &mut Core, shown: bool| {
        let mut ui = core.frame(VIEW, 1.0);
        if shown {
            ui.with_keyed(
                "card",
                NodeSpec::column()
                    .size(100.0, 40.0)
                    .bg(Color::hex(0xff0000ff))
                    .opacity(0.4)
                    .tooltip("a hint")
                    .exit(kui_core::Enter::default().opacity(0.0)),
                |_| {},
            );
        }
        ui.finish();
    };
    card(&mut core, true);
    hover(&mut core, Vec2::new(50.0, 20.0));
    card(&mut core, true);
    card(&mut core, true);
    assert!(close(hint_alpha(&mut core), 1.0), "the live hint, in full");
    core.set_time(0.05);
    card(&mut core, false);
    core.set_time(0.1);
    card(&mut core, false);
    let red = core
        .output()
        .0
        .quads
        .iter()
        .find(|q| q.kind == QuadKind::Solid && q.color.r == 1.0 && q.color.g == 0.0)
        .map(|q| q.color.a);
    assert!(
        red.is_some_and(|a| a > 0.0 && a < 0.4),
        "the ghost fades: {red:?}"
    );
    let a = hint_alpha(&mut core);
    assert!(hidden(a), "and its hint is gone: {a:?}");
}

/// A leaf's tooltip, floated beside the leaf.
fn polygon_leaf(ui: &mut kui_core::Ui<'_>, opacity: f32) {
    let pts = [
        Vec2::new(0.0, 0.0),
        Vec2::new(100.0, 0.0),
        Vec2::new(100.0, 40.0),
        Vec2::new(0.0, 40.0),
    ];
    let mut p = PropsOut::new();
    p.spec = NodeSpec::column().opacity(opacity);
    p.key = Some("leaf".into());
    p.apply_tooltip("a hint");
    ui.core().open_from(p, Content::Polygon(&pts));
}

/// A leaf at opacity 0 is no more there to explain than a box is: its
/// hint, floated beside it, is not drawn either (backlog RG162).
#[test]
fn an_invisible_leafs_tooltip_is_not_drawn() {
    let mut core = Core::new();
    let frame = |core: &mut Core| {
        let mut ui = core.frame(VIEW, 1.0);
        polygon_leaf(&mut ui, 0.0);
        ui.finish();
    };
    frame(&mut core);
    hover(&mut core, Vec2::new(50.0, 20.0));
    frame(&mut core);
    frame(&mut core);
    let a = hint_alpha(&mut core);
    assert!(hidden(a), "no hint over an invisible leaf: {a:?}");
}

/// A hovered leaf inside a box removed with an `exit`: the leaf's hint
/// leaves with the hover, as the box's own does (backlog RG162).
#[test]
fn a_departing_leafs_tooltip_leaves_with_the_hover() {
    let mut core = Core::new();
    core.set_time(0.0);
    let card = |core: &mut Core, shown: bool| {
        let mut ui = core.frame(VIEW, 1.0);
        if shown {
            ui.with_keyed(
                "card",
                NodeSpec::column()
                    .size(100.0, 40.0)
                    .exit(kui_core::Enter::default().opacity(0.0)),
                |ui| polygon_leaf(ui, 1.0),
            );
        }
        ui.finish();
    };
    card(&mut core, true);
    hover(&mut core, Vec2::new(50.0, 20.0));
    card(&mut core, true);
    card(&mut core, true);
    assert!(close(hint_alpha(&mut core), 1.0), "the live hint, in full");
    core.set_time(0.05);
    card(&mut core, false);
    core.set_time(0.1);
    card(&mut core, false);
    let a = hint_alpha(&mut core);
    assert!(hidden(a), "and its hint is gone: {a:?}");
}
