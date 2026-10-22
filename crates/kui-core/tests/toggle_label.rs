//! A stock toggle's label cut to one line (backlog F184): `ellipsis` on a
//! checkbox, a radio or a switch keeps its label on one line, cut with an
//! ellipsis where the toggle is squeezed, part of the click target, named
//! by the whole text, and floated whole as the hint while it is cut and
//! hovered.

use kui_core::testing::{click, hover, messages};
use kui_core::widgets::{self, Toggle};
use kui_core::{Core, NodeSpec, Role, Size, TextStyle, Ui, Value, Vec2};

const VIEW: Size = Size { w: 400.0, h: 300.0 };
const LONG: &str = "third-party-analytics-cookie-with-a-very-long-name and then some more words";

fn cut() -> TextStyle {
    TextStyle {
        ellipsis: true,
        ..TextStyle::default()
    }
}

/// A row 160 px wide holding the toggle and nothing else.
fn frame(core: &mut Core, kind: Toggle, text: &str, label: &TextStyle, hint: Option<&str>) {
    let mut ui = core.frame(VIEW, 1.0);
    row(&mut ui, kind, text, label, hint);
    ui.finish();
}

fn row(ui: &mut Ui<'_>, kind: Toggle, text: &str, label: &TextStyle, hint: Option<&str>) {
    ui.with(NodeSpec::row().width(160.0), |ui| {
        let m = ui.metrics();
        let spec = widgets::toggle_spec(&m).on_click(Value::str("flip"));
        widgets::toggle_with_style(ui, kind, "it", text, spec, hint, label);
    });
}

fn toggle_node(core: &mut Core, role: Role) -> kui_core::access::AccessNode {
    core.access_tree()
        .nodes
        .iter()
        .find(|n| n.role == role)
        .cloned()
        .expect("the toggle is in the tree")
}

/// The width of the hint floated this frame — its surface, a solid quad
/// in the theme's `raised` — or `None` with no hint up. The hint is kept
/// out of the access tree, so its width is what says which text it holds.
fn hint_w(core: &mut Core) -> Option<f32> {
    let raised = core.theme().raised;
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .find(|q| q.kind == kui_core::QuadKind::Solid && q.color == raised)
        .map(|q| q.rect.w)
}

#[test]
fn an_ellipsized_label_stays_on_one_line_inside_the_row() {
    for (kind, role) in [
        (Toggle::Checkbox, Role::Checkbox),
        (Toggle::Radio, Role::Radio),
        (Toggle::Switch, Role::Switch),
    ] {
        let mut core = Core::new();
        frame(&mut core, kind, LONG, &TextStyle::default(), None);
        let wrapped = toggle_node(&mut core, role).rect;

        let mut core = Core::new();
        frame(&mut core, kind, LONG, &cut(), None);
        let one = toggle_node(&mut core, role);
        assert!(
            one.rect.w <= 160.5,
            "{kind:?} held to the row: {:?}",
            one.rect
        );
        assert!(
            one.rect.h < wrapped.h,
            "{kind:?} on one line ({}) where it wrapped ({})",
            one.rect.h,
            wrapped.h
        );
        assert_eq!(
            one.name.as_deref(),
            Some(LONG),
            "{kind:?} named by the whole text"
        );
        let key = core.key_of("it").unwrap();
        assert!(core.text_cut(key), "{kind:?} cut");
    }
}

/// The label is still the click target: a press on its cut end toggles.
#[test]
fn a_press_on_the_cut_label_toggles() {
    let mut core = Core::new();
    frame(&mut core, Toggle::Checkbox, LONG, &cut(), None);
    let r = toggle_node(&mut core, Role::Checkbox).rect;
    let evs = click(&mut core, Vec2::new(r.x + r.w - 4.0, r.y + r.h / 2.0));
    assert_eq!(messages(&evs), ["flip"]);
}

/// Hovered while cut, the whole text floats as the hint; a label that
/// fits floats none; a declared `tooltip` wins over the text.
#[test]
fn the_whole_label_is_the_hint_while_it_is_cut() {
    let mut core = Core::new();
    frame(&mut core, Toggle::Checkbox, LONG, &cut(), None);
    let r = toggle_node(&mut core, Role::Checkbox).rect;
    hover(&mut core, Vec2::new(r.x + r.w / 2.0, r.y + r.h / 2.0));
    frame(&mut core, Toggle::Checkbox, LONG, &cut(), None);
    let whole = hint_w(&mut core).expect("a hint is up");
    assert!(whole > 200.0, "the hint holds the whole label: {whole}");

    let mut core = Core::new();
    frame(&mut core, Toggle::Checkbox, "Short", &cut(), None);
    let r = toggle_node(&mut core, Role::Checkbox).rect;
    let key = core.key_of("it").unwrap();
    assert!(!core.text_cut(key), "a label that fits is not cut");
    hover(&mut core, Vec2::new(r.x + r.w / 2.0, r.y + r.h / 2.0));
    frame(&mut core, Toggle::Checkbox, "Short", &cut(), None);
    assert_eq!(hint_w(&mut core), None, "nothing to show that is not shown");

    let mut core = Core::new();
    frame(
        &mut core,
        Toggle::Checkbox,
        LONG,
        &cut(),
        Some("Says what it does"),
    );
    let r = toggle_node(&mut core, Role::Checkbox).rect;
    hover(&mut core, Vec2::new(r.x + r.w / 2.0, r.y + r.h / 2.0));
    frame(
        &mut core,
        Toggle::Checkbox,
        LONG,
        &cut(),
        Some("Says what it does"),
    );
    let said = hint_w(&mut core).expect("the declared hint is up");
    assert!(
        said < whole,
        "the declared hint ({said}) and not the label ({whole})"
    );
    assert_eq!(
        toggle_node(&mut core, Role::Checkbox)
            .description
            .as_deref(),
        Some("Says what it does")
    );
}

/// Without a line budget nothing changes: the label wraps, and a wrapped
/// label is never reported cut.
#[test]
fn a_plain_label_still_wraps_and_is_never_cut() {
    let mut core = Core::new();
    frame(
        &mut core,
        Toggle::Checkbox,
        LONG,
        &TextStyle::default(),
        None,
    );
    let key = core.key_of("it").unwrap();
    assert!(!core.text_cut(key));
    let r = toggle_node(&mut core, Role::Checkbox).rect;
    hover(&mut core, Vec2::new(r.x + r.w / 2.0, r.y + 4.0));
    frame(
        &mut core,
        Toggle::Checkbox,
        LONG,
        &TextStyle::default(),
        None,
    );
    assert_eq!(hint_w(&mut core), None);
}

/// Two lines and an ellipsis: a clamp, cut at the second line's end.
#[test]
fn max_lines_clamps_the_label() {
    let two = TextStyle {
        max_lines: 2,
        ellipsis: true,
        ..TextStyle::default()
    };
    let mut core = Core::new();
    frame(&mut core, Toggle::Checkbox, LONG, &cut(), None);
    let one = toggle_node(&mut core, Role::Checkbox).rect.h;
    let mut core = Core::new();
    frame(&mut core, Toggle::Checkbox, LONG, &two, None);
    let h = toggle_node(&mut core, Role::Checkbox).rect.h;
    assert!(h > one, "two lines ({h}) are taller than one ({one})");
    let key = core.key_of("it").unwrap();
    assert!(core.text_cut(key));
}
