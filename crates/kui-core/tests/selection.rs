//! Selection scopes through a live `Core`: what a `selectable` container
//! makes one selection of, what a drag across it selects, what a copy
//! reads back, and how far past the viewport either reaches
//! (`docs/adr/0017-selection-as-a-scope.md`).

use kui_core::{
    Core, EditKey, InputEvent, Key, Mods, MouseButton, NodeSpec, QuadKind, Size, Sizing, TextStyle,
    Vec2,
};

fn style() -> TextStyle {
    TextStyle::new(16.0)
}

/// A scope of three labels, one per line, in a column that selects.
fn three_labels(core: &mut Core) -> Key {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let scope = ui.with_keyed(
        "card",
        NodeSpec::column().width(Sizing::Grow(1.0)).selectable(),
        |ui| {
            ui.text("one", style());
            ui.text("two", style());
            ui.text("three", style());
        },
    );
    ui.finish();
    scope
}

#[test]
fn select_all_reads_every_run_in_the_scope() {
    let mut core = Core::new();
    let scope = three_labels(&mut core);
    assert!(core.select_all_in(scope), "the scope drew three runs");
    assert_eq!(core.selection_text().as_deref(), Some("one\ntwo\nthree"));
}

#[test]
fn a_drag_selects_from_one_run_into_another() {
    let mut core = Core::new();
    let scope = three_labels(&mut core);
    // Press at the very start of the first line, drag into the third.
    assert!(core.begin_selection(scope, Vec2::new(0.0, 5.0)));
    assert_eq!(core.selection_text().as_deref(), Some(""));
    assert!(core.extend_selection(Vec2::new(1000.0, 45.0)));
    assert_eq!(core.selection_text().as_deref(), Some("one\ntwo\nthree"));
    // Dragging back to the anchor empties it again without the ends
    // swapping roles.
    assert!(core.extend_selection(Vec2::new(0.0, 5.0)));
    assert_eq!(core.selection_text().as_deref(), Some(""));
}

#[test]
fn a_selection_paints_under_the_glyphs() {
    let mut core = Core::new();
    let scope = three_labels(&mut core);
    core.select_all_in(scope);
    // Selection state alone changes nothing until a frame is built
    // against it, the way every other piece of retained state works.
    let scope2 = three_labels(&mut core);
    assert_eq!(scope, scope2);
    let quads = &core.output().0.quads;
    let tint = kui_core::select::TINT;
    let first_tint = quads.iter().position(|q| q.color == tint);
    let first_glyph = quads.iter().position(|q| q.kind != QuadKind::Solid);
    let tints = quads.iter().filter(|q| q.color == tint).count();
    assert_eq!(tints, 3, "one highlight per run");
    assert!(
        first_tint < first_glyph,
        "the highlight is pushed before the glyphs it sits under"
    );
}

#[test]
fn a_word_is_the_run_of_like_characters_around_the_point() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    // Where "brave" sits, measured rather than guessed: the fonts on a CI
    // container are not the fonts here, and a hard-coded x lands in a
    // different word on a different machine.
    let lead = ui.measure_text("hello ", &style(), None).width;
    let word = ui.measure_text("brave", &style(), None).width;
    let scope = ui.with_keyed(
        "card",
        NodeSpec::column().width(Sizing::Grow(1.0)).selectable(),
        |ui| ui.text("hello brave world", style()),
    );
    ui.finish();
    assert!(
        core.select_word_at(scope, Vec2::new(lead + word / 2.0, 8.0))
            .is_some()
    );
    assert_eq!(core.selection_text().as_deref(), Some("brave"));
}

#[test]
fn a_run_scrolled_out_of_view_still_copies() {
    // Six lines of 20px in a 50px window: the last three never draw.
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let scope = ui.with_keyed(
        "scroll",
        NodeSpec::column()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Fixed(50.0))
            .scroll_y()
            .selectable(),
        |ui| {
            for i in 0..6 {
                ui.text(&format!("row {i}"), style());
            }
        },
    );
    ui.finish();
    assert!(core.select_all_in(scope));
    assert_eq!(
        core.selection_text().as_deref(),
        Some("row 0\nrow 1\nrow 2\nrow 3\nrow 4\nrow 5"),
        "tier 2: a scoped run the frame culled keeps its content and its place"
    );
    // ...but nothing off-screen is under the pointer: a hit far below the
    // clip still lands in the last run that drew.
    let hit = core
        .selection_hit(scope, Vec2::new(0.0, 500.0))
        .expect("a scope with drawn runs answers");
    let ends = core
        .select_all_in(scope)
        .then(|| core.selection())
        .flatten();
    let last_drawn = ends.expect("selection").focus;
    assert_ne!(
        hit.node, last_drawn.node,
        "the last run of the scope is off-screen, so the hit is not in it"
    );
}

#[test]
fn a_long_line_joins_the_concatenation() {
    // Past LONG_LINE_BYTES and non-wrapping: shaped in chunks (C19), and
    // still one run of the scope's text (ADR 0017, open question 1).
    let long = "lorem ipsum ".repeat(500);
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let scope = ui.with_keyed(
        "card",
        NodeSpec::column().width(Sizing::Grow(1.0)).selectable(),
        |ui| {
            ui.text("head", style());
            ui.text(
                &long,
                TextStyle {
                    wrap: kui_core::spec::TextWrap::None,
                    ..style()
                },
            );
        },
    );
    ui.finish();
    assert!(core.select_all_in(scope));
    let text = core.selection_text().expect("a selection");
    assert!(text.starts_with("head\nlorem ipsum"));
    assert_eq!(text.len(), "head\n".len() + long.len());
}

#[test]
fn a_selection_is_the_windows_and_an_editor_gives_it_up() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let mut edit = Key::ROOT;
    let scope = ui.with_keyed(
        "card",
        NodeSpec::column().width(Sizing::Grow(1.0)).selectable(),
        |ui| {
            ui.text("static text", style());
            edit = ui.text_edit(
                "field",
                "typed text",
                &kui_core::EditOptions {
                    autofocus: true,
                    ..Default::default()
                },
                NodeSpec::column().width(Sizing::Grow(1.0)),
            );
        },
    );
    ui.finish();
    core.handle_input(InputEvent::Key(EditKey::SelectAll, Mods::default()));
    assert_eq!(core.copy_selection().as_deref(), Some("typed text"));
    // A selection in the scope takes the window's one selection with it.
    assert!(core.select_all_in(scope));
    assert_eq!(core.copy_selection().as_deref(), Some("static text"));
}

#[test]
fn a_press_and_drag_selects_and_a_press_outside_clears() {
    let mut core = Core::new();
    three_labels(&mut core);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(0.0, 5.0)));
    core.handle_input(InputEvent::MouseDown {
        button: MouseButton::Primary,
        clicks: 1,
    });
    core.handle_input(InputEvent::CursorMoved(Vec2::new(1000.0, 45.0)));
    assert_eq!(core.selection_text().as_deref(), Some("one\ntwo\nthree"));
    core.handle_input(InputEvent::MouseUp {
        button: MouseButton::Primary,
    });
    // The pointer keeps moving after the release without extending it.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(0.0, 5.0)));
    assert_eq!(core.selection_text().as_deref(), Some("one\ntwo\nthree"));
    // A press outside the scope drops it.
    three_labels(&mut core);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(300.0, 250.0)));
    core.handle_input(InputEvent::MouseDown {
        button: MouseButton::Primary,
        clicks: 1,
    });
    assert_eq!(core.selection(), None);
}

#[test]
fn a_double_press_takes_the_word_and_a_triple_the_run() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed(
        "card",
        NodeSpec::column().width(Sizing::Grow(1.0)).selectable(),
        |ui| ui.text("hello brave world", style()),
    );
    ui.finish();
    core.handle_input(InputEvent::CursorMoved(Vec2::new(60.0, 8.0)));
    core.handle_input(InputEvent::MouseDown {
        button: MouseButton::Primary,
        clicks: 2,
    });
    assert_eq!(core.selection_text().as_deref(), Some("brave"));
    core.handle_input(InputEvent::MouseDown {
        button: MouseButton::Primary,
        clicks: 3,
    });
    assert_eq!(core.selection_text().as_deref(), Some("hello brave world"));
}

#[test]
fn a_press_on_a_button_inside_a_scope_clicks_it() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed(
        "card",
        NodeSpec::column().width(Sizing::Grow(1.0)).selectable(),
        |ui| {
            ui.with_keyed(
                "go",
                NodeSpec::column()
                    .width(Sizing::Grow(1.0))
                    .height(Sizing::Fixed(30.0))
                    .on_click(kui_core::Value::str("go")),
                |ui| ui.text("press me", style()),
            );
        },
    );
    ui.finish();
    core.handle_input(InputEvent::CursorMoved(Vec2::new(50.0, 10.0)));
    core.handle_input(InputEvent::MouseDown {
        button: MouseButton::Primary,
        clicks: 1,
    });
    let events = core.handle_input(InputEvent::MouseUp {
        button: MouseButton::Primary,
    });
    assert_eq!(events.len(), 1, "the click still happens: {events:?}");
    assert_eq!(
        core.selection(),
        None,
        "a control claims the press; nothing starts selecting"
    );
}

#[test]
fn a_scope_inside_a_scope_is_warned_about() {
    let mut core = Core::new();
    for _ in 0..2 {
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        ui.with_keyed(
            "outer",
            NodeSpec::column().width(Sizing::Grow(1.0)).selectable(),
            |ui| {
                ui.text("outside", style());
                ui.with_keyed(
                    "inner",
                    NodeSpec::column().width(Sizing::Grow(1.0)).selectable(),
                    |ui| ui.text("inside", style()),
                );
            },
        );
        ui.finish();
    }
    let warnings = core.take_warnings();
    assert!(
        warnings.iter().any(|w| w.code == "nested-selection-scope"),
        "got {warnings:?}"
    );
}

#[test]
fn a_copy_carries_the_formatting_the_text_declared() {
    use kui_core::Span;
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let scope = ui.with_keyed(
        "card",
        NodeSpec::column().width(Sizing::Grow(1.0)).selectable(),
        |ui| {
            ui.rich_text(
                &[
                    Span::new("plain "),
                    Span::new("bold").bold(),
                    Span::new(" & "),
                    Span::new("green").color(kui_core::Color::rgb8(0, 0x80, 0)),
                ],
                style(),
            );
        },
    );
    ui.finish();
    assert!(core.select_all_in(scope));
    assert_eq!(core.selection_text().as_deref(), Some("plain bold & green"));
    let html = core.selection_html().expect("formatting to carry");
    assert!(html.contains("<b>bold</b>"), "{html}");
    assert!(html.contains("color:#008000"), "{html}");
    assert!(html.contains("&amp;"), "escaped, not raw: {html}");
    assert!(
        !html.contains("plain</b>"),
        "the plain run is not swept into the bold one: {html}"
    );
}

#[test]
fn a_plain_selection_carries_no_theme_colour() {
    // The node's own colour is the app's theme, not the text's: a grey
    // paragraph pasted into a white document should not arrive grey.
    let mut core = Core::new();
    let scope = three_labels(&mut core);
    core.select_all_in(scope);
    let html = core.selection_html().expect("some html");
    assert!(!html.contains("color:"), "{html}");
    assert!(html.contains("one<br>two<br>three"), "{html}");
}

/// Double-click and hold, then drag: the selection moves by *words*, and
/// the word the press took stays whole when the drag turns back over it.
/// What every text UI does, and what the stock `<edit>` gets from
/// cosmic-text's `Selection::Word` for nothing.
#[test]
fn a_held_double_press_drags_by_words() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let at = |s: &str, ui: &mut kui_core::Ui<'_>| ui.measure_text(s, &style(), None).width;
    let x_brave = at("hello ", &mut ui) + at("brave", &mut ui) / 2.0;
    let x_world = at("hello brave ", &mut ui) + at("world", &mut ui) / 2.0;
    let x_hello = at("hell", &mut ui);
    ui.with_keyed(
        "card",
        NodeSpec::column().width(Sizing::Grow(1.0)).selectable(),
        |ui| ui.text("hello brave world", style()),
    );
    ui.finish();

    core.handle_input(InputEvent::CursorMoved(Vec2::new(x_brave, 8.0)));
    core.handle_input(InputEvent::MouseDown {
        button: MouseButton::Primary,
        clicks: 2,
    });
    assert_eq!(core.selection_text().as_deref(), Some("brave"));
    // Held and dragged forward: whole words, not the character under the
    // pointer.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(x_world, 8.0)));
    assert_eq!(core.selection_text().as_deref(), Some("brave world"));
    // Back over the start: the word it began in stays whole.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(x_hello, 8.0)));
    assert_eq!(core.selection_text().as_deref(), Some("hello brave"));
    core.handle_input(InputEvent::MouseUp {
        button: MouseButton::Primary,
    });
    // The release ends the drag: moving on does not keep selecting.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(x_world, 8.0)));
    assert_eq!(core.selection_text().as_deref(), Some("hello brave"));
}

/// The same gesture one click deeper: a third press held drags whole runs.
#[test]
fn a_held_triple_press_drags_by_runs() {
    let mut core = Core::new();
    let scope = three_labels(&mut core);
    let _ = scope;
    core.handle_input(InputEvent::CursorMoved(Vec2::new(4.0, 8.0)));
    core.handle_input(InputEvent::MouseDown {
        button: MouseButton::Primary,
        clicks: 3,
    });
    assert_eq!(core.selection_text().as_deref(), Some("one"));
    // Into the third label: whole runs, not a partial one.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(4.0, 45.0)));
    assert_eq!(core.selection_text().as_deref(), Some("one\ntwo\nthree"));
}

/// One click still drags by characters — the gesture that was there
/// before, unchanged.
#[test]
fn a_single_press_still_drags_by_characters() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let lead = ui.measure_text("hello ", &style(), None).width;
    let two = ui.measure_text("hello br", &style(), None).width;
    ui.with_keyed(
        "card",
        NodeSpec::column().width(Sizing::Grow(1.0)).selectable(),
        |ui| ui.text("hello brave world", style()),
    );
    ui.finish();
    core.handle_input(InputEvent::CursorMoved(Vec2::new(lead, 8.0)));
    core.handle_input(InputEvent::MouseDown {
        button: MouseButton::Primary,
        clicks: 1,
    });
    core.handle_input(InputEvent::CursorMoved(Vec2::new(two, 8.0)));
    assert_eq!(core.selection_text().as_deref(), Some("br"));
}
