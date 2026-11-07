use kui_native::{App, Core, CursorShape, InputEvent, KeyMods, NodeSpec, Ui, Vec2};

#[derive(Default)]
struct Hover {
    built: usize,
    observed: usize,
}

impl App for Hover {
    fn view(&mut self, ui: &mut Ui<'_>) {
        self.built += 1;
        ui.leaf_keyed("editor", NodeSpec::column().fill().on_key("edit"));
    }

    fn after_input(&mut self, core: &mut Core) {
        self.observed += 1;
        let key = core.key_of("editor").unwrap();
        core.set_cursor(
            key,
            core.modifiers().super_key.then_some(CursorShape::Pointer),
        );
    }
}

#[test]
fn unhandled_modifiers_update_the_cursor_without_building_a_frame() {
    let mut app = Hover::default();
    let mut drive = kui_native::testing::Drive::new(Core::new(), 200.0, 100.0);
    drive.frame(&mut app);
    drive.input(&mut app, InputEvent::CursorMoved(Vec2::new(20.0, 20.0)));
    let events = drive.input(
        &mut app,
        InputEvent::Modifiers(KeyMods {
            super_key: true,
            ..KeyMods::NONE
        }),
    );
    assert!(events.is_empty());
    assert_eq!(drive.core.cursor_shape(), CursorShape::Pointer);
    drive.input(&mut app, InputEvent::Modifiers(KeyMods::NONE));
    assert_eq!(drive.core.cursor_shape(), CursorShape::Default);
    assert_eq!(app.built, 1);
    assert_eq!(app.observed, 3);
}
