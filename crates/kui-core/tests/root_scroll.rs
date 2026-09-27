use kui_core::{Color, Core, InputEvent, NodeSpec, Size, TextStyle, Vec2};

#[test]
fn root_scroll_works() {
    let mut core = Core::new();
    let build = |core: &mut Core| {
        let mut ui = core.frame(Size::new(560.0, 360.0), 1.0);
        ui.configure_root(NodeSpec::column().fill().pad(24.0).scroll_y());
        ui.with(
            NodeSpec::column()
                .grow_width()
                .max_width(560.0)
                .pad(36.0)
                .gap(18.0)
                .bg(Color::rgb8(20, 20, 30)),
            |ui| {
                for i in 0..10 {
                    ui.text(&format!("paragraph line {i}"), TextStyle::new(24.0));
                }
            },
        );
        ui.finish();
    };
    build(&mut core);
    let (dl, _) = core.output();
    let bars = dl.quads.iter().filter(|q| q.rect.w == 4.0).count();
    let first_y = dl
        .quads
        .iter()
        .find(|q| q.kind == kui_core::QuadKind::Solid && q.rect.w > 100.0)
        .map(|q| q.rect.y);
    println!("bars={bars} card_y={first_y:?}");
    assert_eq!(bars, 1, "root scrollbar missing");

    core.handle_input(InputEvent::CursorMoved(Vec2::new(280.0, 180.0)));
    core.handle_input(InputEvent::Scroll(Vec2::new(0.0, -100.0)));
    build(&mut core);
    let (dl, _) = core.output();
    let card_y = dl
        .quads
        .iter()
        .find(|q| q.kind == kui_core::QuadKind::Solid && q.rect.w > 100.0)
        .map(|q| q.rect.y);
    println!("after scroll card_y={card_y:?}");
    assert_eq!(card_y, Some(24.0 - 100.0));
}
