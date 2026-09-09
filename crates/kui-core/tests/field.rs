//! An `<edit>` measures what its text wants, not what the last frame's
//! wrap left of it. Backlog F38.

use kui_core::{Core, EditOptions, InputEvent, Key, NodeSpec, Size, Sizing, TextStyle};

const PAD: f32 = 5.0;

/// One editor in a roomy root. `width` is the box's, `None` = fit its text.
fn frame(
    core: &mut Core,
    initial: &str,
    multiline: bool,
    width: Option<f32>,
    size: f32,
) -> (Key, kui_core::Rect) {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().pad(10.0));
    let spec = NodeSpec::column().pad(PAD).width(match width {
        Some(w) => Sizing::Fixed(w),
        None => Sizing::Fit,
    });
    let key = ui.text_edit(
        "field",
        initial,
        &EditOptions {
            multiline,
            autofocus: true,
            style: TextStyle::new(size),
            ..Default::default()
        },
        spec,
    );
    ui.finish();
    let rect = core
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.key == key)
        .map(|n| n.rect)
        .expect("the editor is in the access tree");
    (key, rect)
}

/// How many visual lines the editor's text is on, read off the access
/// tree's runs (one run per visual line).
fn lines(core: &mut Core, key: Key) -> usize {
    core.access_tree()
        .nodes
        .iter()
        .find(|n| n.key == key)
        .map_or(0, |n| n.runs.len())
}

#[test]
fn a_field_that_hugs_its_text_grows_with_it() {
    // F38: the fit width used to be measured off a buffer still carrying
    // the last frame's wrap, so the box took the widest wrapped line, the
    // next frame wrapped to that, and a field seeded empty ratcheted down
    // to one character with every keystroke on its own line.
    let mut core = Core::new();
    let (key, empty) = frame(&mut core, "", false, None, 16.0);
    let mut last = empty.w;
    for ch in ["h", "e", "l", "l", "o"] {
        core.handle_input(InputEvent::Text(ch.to_string()));
        let (_, rect) = frame(&mut core, "", false, None, 16.0);
        assert_eq!(lines(&mut core, key), 1, "a field stays on one line");
        assert!(
            rect.w >= last,
            "the box must not narrow as the text grows: {last} -> {}",
            rect.w
        );
        last = rect.w;
    }
    assert_eq!(core.edit_text(key).as_deref(), Some("hello"));
    assert!(
        last > empty.w + 20.0,
        "five characters should have widened the box: {} -> {last}",
        empty.w
    );
}

#[test]
fn a_style_change_remeasures_the_same_text() {
    // The measurement caches are keyed on the metrics as well as the text:
    // the same string at 32px is not the size it was at 16.
    let mut core = Core::new();
    let (_, small) = frame(&mut core, "Throwaway", false, None, 16.0);
    let (_, big) = frame(&mut core, "", false, None, 32.0);
    assert!(
        big.w > small.w * 1.5,
        "a doubled font should about double the box: {} -> {}",
        small.w,
        big.w
    );
}
