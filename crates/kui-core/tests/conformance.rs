//! The Rust adapter over the scene corpus (`kui_core::conformance`).
//!
//! Two jobs. It pins the reference behaviour against the font-independent
//! half of each scene's expectations — the half that can be checked in and
//! still hold on someone else's machine. And it checks that the corpus
//! covers every `schema::CUSTOM` and `schema::ELEMENTS` row, so a new
//! hand-written prop or element cannot be added without a scene that the
//! Lua, C and Node adapters then have to reproduce.

use std::collections::BTreeSet;

use kui_core::conformance::{self, Output, Scene};
use kui_core::schema::{CUSTOM, ELEMENTS};

/// The checked-in spelling of an access row: everything about it that does
/// not move with the installed fonts.
fn access_rows(out: &Output) -> Vec<String> {
    out.nodes
        .iter()
        .map(|n| {
            format!(
                "{} {} {}|{}|{}",
                n.depth, n.role, n.name, n.description, n.value
            )
        })
        .collect()
}

fn events(out: &Output) -> Vec<String> {
    out.events
        .iter()
        .map(|(kind, tag)| format!("{kind} {tag}"))
        .collect()
}

fn check(scene: &Scene) {
    let out = conformance::run(scene);
    let e = &scene.expect;
    let name = scene.name;
    assert_eq!(out.kinds[0], e.solid, "{name}: solid quads");
    assert_eq!(out.kinds[5], e.shadows, "{name}: shadow quads");
    assert_eq!(out.kinds[3], e.images, "{name}: image quads");
    let glyphs = out.kinds[1] + out.kinds[2] + out.kinds[4];
    assert!(
        glyphs >= e.glyphs_min,
        "{name}: {glyphs} glyph quads, expected at least {}",
        e.glyphs_min
    );
    assert_eq!(access_rows(&out), e.access, "{name}: access tree");
    assert_eq!(events(&out), e.events, "{name}: events");
    assert_eq!(out.warnings, e.warnings, "{name}: warnings");
    assert_eq!(out.title.as_deref(), e.title, "{name}: window title");
}

#[test]
fn every_scene_matches_its_expectations() {
    for scene in conformance::SCENES {
        check(scene);
    }
}

/// A scene that draws nothing would pass every count above by accident.
#[test]
fn every_scene_draws_and_reports() {
    for scene in conformance::SCENES {
        let out = conformance::run(scene);
        assert!(out.quad_count > 0, "{}: no quads", scene.name);
        assert!(!out.nodes.is_empty(), "{}: no access tree", scene.name);
        let block = conformance::report(scene.name, scene.steps, &out);
        assert!(block.starts_with(&format!("scene {}\n", scene.name)));
        assert!(block.ends_with("end\n"));
    }
}

/// Scene names are the corpus's index across four languages: an adapter
/// looks its scenes up by name, so duplicates would silently shadow.
#[test]
fn scene_names_are_unique() {
    let names: BTreeSet<_> = conformance::SCENES.iter().map(|s| s.name).collect();
    assert_eq!(names.len(), conformance::SCENES.len());
}

/// The point of the corpus: `CUSTOM` and `ELEMENTS` stop being a document.
#[test]
fn the_corpus_covers_every_hand_written_row() {
    let custom: BTreeSet<&str> = conformance::SCENES
        .iter()
        .flat_map(|s| s.custom.iter().copied())
        .collect();
    let elements: BTreeSet<&str> = conformance::SCENES
        .iter()
        .flat_map(|s| s.elements.iter().copied())
        .collect();

    let missing: Vec<&str> = CUSTOM
        .iter()
        .map(|c| c.name)
        .filter(|n| !custom.contains(n))
        .collect();
    assert!(
        missing.is_empty(),
        "no scene exercises these schema::CUSTOM rows: {missing:?}"
    );
    let missing: Vec<&str> = ELEMENTS
        .iter()
        .map(|e| e.name)
        .filter(|n| !elements.contains(n))
        .collect();
    assert!(
        missing.is_empty(),
        "no scene exercises these schema::ELEMENTS rows: {missing:?}"
    );

    // And the other way: a scene naming a row that no longer exists is a
    // stale claim of coverage.
    for name in &custom {
        assert!(
            CUSTOM.iter().any(|c| &c.name == name),
            "scene claims schema::CUSTOM row {name:?}, which does not exist"
        );
    }
    for name in &elements {
        assert!(
            ELEMENTS.iter().any(|e| &e.name == name),
            "scene claims schema::ELEMENTS row {name:?}, which does not exist"
        );
    }
}

/// Every binding drives the same protocol, so the reference dump has to be
/// reproducible in one process: same scenes, same order, same bytes.
#[test]
fn the_reference_report_is_deterministic() {
    assert_eq!(
        conformance::reference_report(),
        conformance::reference_report()
    );
    let blocks = conformance::blocks(&conformance::reference_report());
    assert_eq!(blocks.len(), conformance::SCENES.len());
    for (scene, (name, _)) in conformance::SCENES.iter().zip(&blocks) {
        assert_eq!(scene.name, name);
    }
}
