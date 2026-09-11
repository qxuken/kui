//! The sizes the stock widgets are built from (`kui_core::metrics`,
//! backlog T2): the stock set is the constants they always had, a set the
//! app chooses reaches every widget, and nothing scales by itself.

use kui_core::geom::{Size, Vec2};
use kui_core::menu::MenuItem;
use kui_core::{Core, Metrics, NodeSpec, QuadKind, Rect, Value, widgets};

/// The stock widgets' boxes, in paint order: the button, the field, the
/// tooltip, the menu panel and its first row.
fn boxes(core: &mut Core) -> Vec<Rect> {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    let spec = widgets::button_spec(&ui.metrics()).on_click(Value::str("ok"));
    widgets::button_with(&mut ui, "ok", "OK", spec, None);
    widgets::text_input(&mut ui, "search", "hello");
    widgets::tooltip(&mut ui, "a hint");
    widgets::context_menu(&mut ui, Vec2::new(10.0, 10.0), &[MenuItem::new("Copy")]);
    ui.finish();
    let (list, _) = core.output();
    list.quads
        .iter()
        .filter(|q| q.kind == QuadKind::Solid)
        .map(|q| q.rect)
        .collect()
}

/// The stock set is what the widgets drew before there was a struct: the
/// constants that stayed public are its values.
#[test]
fn the_stock_set_is_the_constants() {
    let m = Metrics::default();
    assert_eq!(m, Metrics::comfortable());
    assert_eq!(m.control_text, widgets::BUTTON_TEXT);
    assert_eq!(m.chrome_text, widgets::MENU_TEXT);
    assert_eq!(m.menu_width, widgets::MENU_WIDTH);
    assert_eq!(m.menu_bar_h, widgets::MENU_BAR_H);
    assert_eq!(m.titlebar_h, widgets::TITLEBAR_H);
    assert_eq!(
        (m.control_pad_x, m.control_pad_y, m.radius),
        (14.0, 8.0, 6.0)
    );
}

/// A set the app chooses reaches every stock widget: compact shrinks the
/// button, the field, the tooltip and the menu, and the corners with them.
#[test]
fn a_compact_set_reaches_every_widget() {
    let mut core = Core::new();
    let stock = boxes(&mut core);
    core.set_metrics(Metrics::compact());
    assert_eq!(*core.metrics(), Metrics::compact());
    let compact = boxes(&mut core);
    assert_eq!(stock.len(), compact.len());
    let (button, field, hint, menu) = (0, 1, 2, 3);
    for i in [button, field, hint] {
        assert!(
            compact[i].h < stock[i].h && compact[i].w <= stock[i].w,
            "widget {i}: {:?} is not smaller than {:?}",
            compact[i],
            stock[i]
        );
    }
    assert_eq!(compact[menu].w, Metrics::compact().menu_width);
    assert_eq!(stock[menu].w, Metrics::default().menu_width);
    // The button's label shrinks with `control_text`.
    let glyph_h = |core: &mut Core| {
        boxes(core);
        core.output()
            .0
            .quads
            .iter()
            .find(|q| q.kind == QuadKind::GlyphMask)
            .map(|q| q.rect.h)
            .unwrap()
    };
    let small = glyph_h(&mut core);
    core.set_metrics(Metrics::default());
    assert!(glyph_h(&mut core) > small);
}

/// `scaled` multiplies every length and nothing else does: the renderer's
/// scale factor is applied after, so the same metrics at scale 2 draw the
/// same logical box, twice as many pixels.
#[test]
fn scaling_is_the_apps_not_the_scale_factors() {
    let m = Metrics::default().scaled(1.5);
    assert_eq!(m.radius, 9.0);
    assert_eq!(m.control_pad_x, 21.0);
    assert_eq!(m.control_text, 22.5);
    let mut core = Core::new();
    let one = boxes(&mut core);
    let mut ui = core.frame(Size::new(400.0, 300.0), 2.0);
    let spec = widgets::button_spec(&ui.metrics()).on_click(Value::str("ok"));
    widgets::button_with(&mut ui, "ok", "OK", spec, None);
    ui.finish();
    let two = core.output().0.quads[0].rect;
    assert_eq!((two.w, two.h), (one[0].w * 2.0, one[0].h * 2.0));
}

/// The compact set keeps the platform's titlebar: that number is the
/// OS's, not a density.
#[test]
fn compact_keeps_the_platforms_titlebar() {
    assert_eq!(Metrics::compact().titlebar_h, Metrics::default().titlebar_h);
    // And the titlebar draws from the metrics, not the constant: a scaled
    // set makes it taller. Its box is read off a child that fills it.
    let bar_h = |core: &mut Core| {
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        widgets::titlebar_with(&mut ui, |ui| {
            ui.with(
                NodeSpec::column()
                    .width(kui_core::Sizing::Fixed(10.0))
                    .height(kui_core::Sizing::Grow(1.0))
                    .bg(kui_core::Color::WHITE),
                |_| {},
            );
        });
        ui.finish();
        core.output().0.quads[0].rect.h
    };
    let mut core = Core::new();
    assert_eq!(bar_h(&mut core), widgets::TITLEBAR_H);
    core.set_metrics(Metrics::compact().scaled(2.0));
    assert_eq!(bar_h(&mut core), widgets::TITLEBAR_H * 2.0);
}
