//! FZ14 and FZ18. A neutral run cached in one direction must shape in the
//! direction of its next paragraph, including another window of a session.

use kui_core::{Core, Key, NodeSpec, Rect, Session, Size, TextStyle};

fn session() -> Session {
    Session::with_fonts(
        "en-US",
        [
            include_bytes!("../../kui-cosmic-text/fonts/NotoSans-Regular.ttf").to_vec(),
            include_bytes!("../../kui-cosmic-text/fonts/NotoSansHebrew.ttf").to_vec(),
        ],
    )
}

fn draw(core: &mut Core, text: &str) -> Vec<Rect> {
    let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.text(text, TextStyle::new(20.0));
    ui.finish();
    text.char_indices()
        .map(|(byte, _)| byte)
        .chain(std::iter::once(text.len()))
        .map(|byte| {
            core.caret_rect(Key::ROOT.index(0), byte)
                .expect("drawn caret")
        })
        .collect()
}

#[test]
fn neutral_runs_use_their_current_direction_in_the_same_window() {
    let mut fresh = Core::new_in(&session());
    let expected = draw(&mut fresh, "]] b");
    assert!(expected[0].x < expected[1].x && expected[1].x < expected[2].x);

    let mut kept = Core::new_in(&session());
    draw(&mut kept, "א k]] א");
    assert_eq!(draw(&mut kept, "]] b"), expected);
}

#[test]
fn neutral_runs_use_their_current_direction_in_another_window() {
    let mut fresh = Core::new_in(&session());
    let expected = draw(&mut fresh, "]] b");

    let shared = session();
    let mut first = Core::new_in(&shared);
    let mut second = Core::new_in(&shared);
    draw(&mut first, "א k]] א");
    assert_eq!(draw(&mut second, "]] b"), expected);
}
