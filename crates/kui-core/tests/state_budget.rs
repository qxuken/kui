//! The ceiling on retained editor and scroll state (backlog F26).
//!
//! Both stores keep state by key across the key's absence — that is what
//! the `<edit>` row promises and what a tabbed form returning to a field
//! relies on — so neither prunes on undeclare. What they do instead is cap
//! the *undeclared* ones: past the budget the longest-undeclared entry is
//! evicted first, and a state the last frame declared is never evicted.

use kui_core::{
    Color, Core, EditOptions, Key, MAX_UNDECLARED_EDITS, MAX_UNDECLARED_SCROLLS, NodeSpec, Size,
    Vec2,
};

/// One frame declaring editors `first..last`, each seeded with its index.
fn edit_frame(core: &mut Core, range: std::ops::Range<usize>) -> Vec<Key> {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let keys = range
        .map(|i| {
            ui.text_edit(
                &format!("f{i}"),
                &format!("draft {i}"),
                &EditOptions::default(),
                NodeSpec::column().grow_width(),
            )
        })
        .collect();
    ui.finish();
    keys
}

/// One frame declaring scroll containers `first..last`.
fn scroll_frame(core: &mut Core, range: std::ops::Range<usize>) -> Vec<Key> {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let keys = range
        .map(|i| {
            ui.with_keyed(
                &format!("s{i}"),
                NodeSpec::column()
                    .grow_width()
                    .height(10.0)
                    .bg(Color::rgb8(20, 20, 20))
                    .scroll_y(),
                // Something to scroll: an offset the layout cannot clamp
                // to zero is how a surviving entry proves it is the same
                // entry.
                |ui| {
                    ui.leaf_keyed("tall", NodeSpec::column().grow_width().height(100.0));
                },
            )
        })
        .collect();
    ui.finish();
    keys
}

/// The promise the budget must not break: an editor nobody declares keeps
/// its draft, for as long as the store is inside its budget.
#[test]
fn an_undeclared_editor_keeps_its_draft() {
    let mut core = Core::new();
    let keys = edit_frame(&mut core, 0..3);
    core.set_edit_text(keys[1], "typed by hand");
    for _ in 0..500 {
        edit_frame(&mut core, 0..1);
    }
    assert_eq!(core.edit.len(), 3, "nothing is pruned on undeclare");
    assert_eq!(core.edit_text(keys[1]).as_deref(), Some("typed by hand"));
    assert_eq!(core.edit_text(keys[2]).as_deref(), Some("draft 2"));
}

/// Budget + 1 + a margin declared once, then one of them declared forever:
/// the store settles at budget + 1 (the budget's worth of undeclared ones,
/// plus the declared one), and the declared one is never what goes.
#[test]
fn undeclared_editors_are_capped_and_the_declared_one_survives() {
    let mut core = Core::new();
    let n = MAX_UNDECLARED_EDITS + 50;
    let keys = edit_frame(&mut core, 0..n);
    assert_eq!(core.edit.len(), n, "all of them exist to begin with");
    // The survivor is the first key declared, which is also the oldest and
    // the first the eviction order would reach if being declared did not
    // save it.
    core.set_edit_text(keys[0], "kept");
    for _ in 0..100 {
        edit_frame(&mut core, 0..1);
    }
    assert_eq!(
        core.edit.len(),
        MAX_UNDECLARED_EDITS + 1,
        "the budget's worth of undeclared editors, plus the declared one"
    );
    assert_eq!(
        core.edit_text(keys[0]).as_deref(),
        Some("kept"),
        "a declared editor is never evicted"
    );
    // The eviction is by age, and every undeclared one here is the same
    // age, so which of them went is the key order — but 50 of them did.
    let gone = keys[1..].iter().filter(|k| core.edit_text(**k).is_none());
    assert_eq!(gone.count(), n - MAX_UNDECLARED_EDITS - 1);
}

/// Declaring stops as soon as the store is back at its budget: a second
/// wave of editors evicts the first wave's leftovers rather than growing.
#[test]
fn a_second_wave_of_editors_does_not_grow_the_store() {
    let mut core = Core::new();
    let n = MAX_UNDECLARED_EDITS + 50;
    edit_frame(&mut core, 0..n);
    for _ in 0..100 {
        edit_frame(&mut core, 0..1);
    }
    let after_first = core.edit.len();
    edit_frame(&mut core, n..(n + 100));
    for _ in 0..100 {
        edit_frame(&mut core, 0..1);
    }
    assert_eq!(
        core.edit.len(),
        after_first,
        "the store settles at the same ceiling however many waves pass through"
    );
}

#[test]
fn an_undeclared_scroll_offset_survives_absence() {
    let mut core = Core::new();
    let keys = scroll_frame(&mut core, 0..3);
    core.set_scroll(keys[2], Vec2::new(0.0, 40.0));
    for _ in 0..500 {
        scroll_frame(&mut core, 0..1);
    }
    assert_eq!(core.scroll.len(), 3);
    assert_eq!(core.scroll_offset(keys[2]), Vec2::new(0.0, 40.0));
}

#[test]
fn undeclared_scroll_entries_are_capped_and_the_declared_one_survives() {
    let mut core = Core::new();
    let n = MAX_UNDECLARED_SCROLLS + 50;
    let keys = scroll_frame(&mut core, 0..n);
    assert_eq!(core.scroll.len(), n);
    core.set_scroll(keys[0], Vec2::new(0.0, 7.0));
    for _ in 0..100 {
        scroll_frame(&mut core, 0..1);
    }
    assert_eq!(core.scroll.len(), MAX_UNDECLARED_SCROLLS + 1);
    assert_eq!(
        core.scroll_offset(keys[0]),
        Vec2::new(0.0, 7.0),
        "a declared container is never evicted"
    );
}
