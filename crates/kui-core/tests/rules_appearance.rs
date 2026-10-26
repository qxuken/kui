//! The appearance an origin's `dark` / `light` conditions read (backlog
//! C113): the palette's, unless the origin set its own — an add-on that
//! holds its look light or dark has its rules agree with its colours.

use kui_core::{Appearance, Color, Cond, Core, NodeSpec, Size, testing};

fn frame(core: &mut Core) -> Color {
    let mut ui = core.frame(Size::new(100.0, 100.0), 1.0);
    ui.leaf_keyed(
        "box",
        NodeSpec::row()
            .size(10.0, 10.0)
            .bg(Color::WHITE)
            .when(Cond::Dark, |p| p.bg(Color::BLACK)),
    );
    ui.finish();
    testing::solids(core)[0].color
}

#[test]
fn an_origin_sets_the_appearance_its_conditions_read() {
    let mut core = Core::new();
    core.set_theme(kui_core::Theme::light());
    assert_eq!(frame(&mut core), Color::WHITE, "the palette is light");
    core.set_rules_appearance(Appearance::Dark);
    assert_eq!(core.rules_appearance(), Appearance::Dark);
    assert_eq!(frame(&mut core), Color::BLACK, "the host said dark");
    core.set_rules_appearance(Appearance::Unknown);
    assert_eq!(frame(&mut core), Color::WHITE, "the palette's again");
}
