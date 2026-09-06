//! `Core::key_of`: a declared label resolves to its node's key from
//! outside the build, where the path from the root — the auto-keyed
//! ancestors a script cannot spell — is not known (backlog F5).

use kui_core::diag::AMBIGUOUS_KEY;
use kui_core::{Core, EditOptions, Key, NodeSpec, Size, Sizing};

fn cell() -> NodeSpec {
    NodeSpec::column()
        .width(Sizing::Fixed(40.0))
        .height(Sizing::Fixed(20.0))
        .focusable()
}

/// Two auto-keyed levels, then labelled leaves: `alpha`, `beta`, and
/// (when `twice`) a second `beta` under another unkeyed parent.
fn frame(core: &mut Core, twice: bool) -> (Key, Key, Key) {
    let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
    let mut keys = (Key(0), Key(0), Key(0));
    ui.with(NodeSpec::row(), |ui| {
        ui.with(NodeSpec::column(), |ui| {
            keys.0 = ui.with_keyed("alpha", cell(), |_| {});
            keys.1 = ui.with_keyed("beta", cell(), |_| {});
        });
        if twice {
            ui.with(NodeSpec::column(), |ui| {
                keys.2 = ui.with_keyed("beta", cell(), |_| {});
            });
        }
    });
    ui.finish();
    keys
}

#[test]
fn a_label_resolves_to_the_key_the_build_gave_it() {
    let mut core = Core::new();
    let (alpha, beta, _) = frame(&mut core, false);
    assert_eq!(core.key_of("alpha"), Some(alpha));
    assert_eq!(core.key_of("beta"), Some(beta));
    // The same key the path spells: two auto-keyed ancestors, then the label.
    assert_eq!(beta, Key::ROOT.index(0).index(0).str("beta"));
    assert_eq!(core.key_of("gamma"), None);
    assert_eq!(core.key_of(""), None, "an empty label matches nothing");
    assert!(core.take_warnings().is_empty());

    // Usable: a node never interacted with takes focus by its label.
    let k = core.key_of("beta").unwrap();
    core.set_focus(Some(k));
    assert_eq!(core.focus(), Some(beta));
}

/// The editor is a leaf keyed by its label rather than a container
/// opened under one, and "focus the field I just created" is the case
/// the finding came from.
#[test]
fn an_editor_resolves_by_its_label_too() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
    let mut edit = Key(0);
    ui.with(NodeSpec::column(), |ui| {
        edit = ui.text_edit("note", "", &EditOptions::default(), NodeSpec::column());
    });
    ui.finish();
    assert_eq!(core.key_of("note"), Some(edit));
    let by_label = core.key_of("note");
    core.set_focus(by_label);
    assert_eq!(core.focus(), Some(edit));
}

#[test]
fn two_nodes_on_one_label_warn_once_and_the_first_wins() {
    let mut core = Core::new();
    let (_, first, second) = frame(&mut core, true);
    assert_ne!(first, second, "distinct parents, distinct keys");
    assert_eq!(core.key_of("beta"), Some(first));
    assert_eq!(core.key_of("beta"), Some(first));
    let ws = core.take_warnings();
    assert_eq!(ws.len(), 1, "{ws:?}");
    assert_eq!(ws[0].code, AMBIGUOUS_KEY);
    assert_eq!(ws[0].key, first);
    assert!(ws[0].message.contains("2 nodes") && ws[0].message.contains("\"beta\""), "{}", ws[0].message);
    // A label declared once never warns, whatever else the frame declares.
    assert_eq!(core.key_of("alpha").is_some(), true);
    assert!(core.take_warnings().is_empty());
}

#[test]
fn the_index_is_per_frame() {
    let mut core = Core::new();
    frame(&mut core, true);
    frame(&mut core, false);
    let (alpha, beta, _) = frame(&mut core, false);
    assert_eq!(core.key_of("beta"), Some(beta), "the second `beta` is two frames gone");
    assert!(core.take_warnings().is_empty(), "and no longer ambiguous");
    // A frame that drops the label drops the answer.
    let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
    ui.with_keyed("alpha", cell(), |_| {});
    ui.finish();
    assert_eq!(core.key_of("alpha"), Some(Key::ROOT.str("alpha")));
    assert_ne!(core.key_of("alpha"), Some(alpha), "a moved node is its new key");
    assert_eq!(core.key_of("beta"), None);
}

#[test]
fn while_a_frame_is_built_it_reads_this_frame_first_and_then_the_last() {
    let mut core = Core::new();
    let (alpha, beta, _) = frame(&mut core, false);
    let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
    // Not declared yet this frame: the last finished frame answers, which
    // is what lets a view focus a row it declared last frame.
    assert_eq!(ui.key_of("beta"), Some(beta));
    // Declared this frame, elsewhere: this frame's key wins over last
    // frame's, since it is the one the frame being built will have.
    let moved = ui.with_keyed("alpha", cell(), |_| {});
    assert_ne!(moved, alpha);
    assert_eq!(ui.key_of("alpha"), Some(moved));
    // Never declared: nothing.
    assert_eq!(ui.key_of("gamma"), None);
    ui.finish();
    assert_eq!(core.key_of("beta"), None, "finished: last frame no longer answers");
    assert_eq!(core.key_of("alpha"), Some(moved));
    assert!(core.take_warnings().is_empty());
}
