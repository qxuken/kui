//! The main axis's spreads and the cross axis's baselines (backlog C13),
//! and a declared aspect ratio (C14), through a real `Core`: the solver's
//! arithmetic is pinned by `layout.rs`'s own tests against a stub; what is
//! left to check here is that real text measures a baseline, and the two
//! warnings.

use kui_core::diag::{ALIGN_IGNORED, ASPECT_IGNORED};
use kui_core::spec::{Align, FloatConfig, NodeSpec, Sizing};
use kui_core::testing::codes;
use kui_core::{Core, Size, TextStyle};

fn frame(core: &mut Core, build: impl FnOnce(&mut kui_core::Ui<'_>)) {
    let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
    build(&mut ui);
    ui.finish();
}

/// A 12 px and a 32 px text on a baseline row: the small one sits lower,
/// and — since the large one's descender is the longer — its bottom edge
/// is higher. Aligned by bottom edges (a measurer that knew no baseline)
/// the two bottoms would meet.
#[test]
fn real_text_lines_up_by_its_baseline() {
    let mut core = Core::new();
    core.set_inspect(true);
    frame(&mut core, |ui| {
        ui.with(NodeSpec::row().cross_align(Align::Baseline), |ui| {
            ui.text("small", TextStyle::new(12.0));
            ui.text("LARGE", TextStyle::new(32.0));
        });
    });
    let nodes = core.nodes();
    let rect = |t: &str| {
        nodes
            .iter()
            .find(|n| n.text.as_deref() == Some(t))
            .unwrap_or_else(|| panic!("{t} is in the snapshot"))
            .rect
    };
    let (small, large) = (rect("small"), rect("LARGE"));
    assert!(small.y > large.y, "{small:?} {large:?}");
    assert!(
        small.y + small.h < large.y + large.h - 0.5,
        "bottoms met, so the texts aligned by their edges: {small:?} {large:?}"
    );
}

fn warnings_for(spec: NodeSpec) -> Vec<kui_core::diag::Warning> {
    let mut core = Core::new();
    frame(&mut core, |ui| {
        ui.with(spec, |ui| {
            ui.leaf(
                NodeSpec::column()
                    .width(Sizing::Fixed(10.0))
                    .height(Sizing::Fixed(10.0)),
            );
        });
    });
    core.take_warnings()
}

#[test]
fn an_alignment_on_the_wrong_axis_warns() {
    for (spec, says) in [
        (
            NodeSpec::row().main_align(Align::Baseline),
            "mainAlign baseline",
        ),
        (
            NodeSpec::row().cross_align(Align::SpaceBetween),
            "crossAlign spaceBetween",
        ),
        (
            NodeSpec::column().cross_align(Align::Baseline),
            "on a column",
        ),
        (
            NodeSpec::column().float(FloatConfig::below().at(Align::SpaceEvenly, Align::End)),
            "not spaceEvenly",
        ),
    ] {
        let ws = warnings_for(spec);
        assert_eq!(codes(&ws), [ALIGN_IGNORED]);
        assert!(ws[0].message.contains(says), "{}", ws[0].message);
    }
}

#[test]
fn an_alignment_where_it_means_something_says_nothing() {
    for spec in [
        NodeSpec::row().main_align(Align::SpaceBetween),
        NodeSpec::column().main_align(Align::SpaceEvenly),
        NodeSpec::row().cross_align(Align::Baseline),
    ] {
        assert!(warnings_for(spec).is_empty());
    }
}

#[test]
fn a_ratio_with_no_fit_axis_to_size_warns() {
    for (spec, says) in [
        (
            NodeSpec::column()
                .width(Sizing::Fixed(40.0))
                .height(Sizing::Fixed(40.0))
                .aspect_ratio(2.0),
            "both axes are declared",
        ),
        (
            NodeSpec::column()
                .height(Sizing::Grow(1.0))
                .aspect_ratio(2.0),
            "resolved only after every width",
        ),
    ] {
        let ws = warnings_for(spec);
        assert_eq!(codes(&ws), [ASPECT_IGNORED]);
        assert!(ws[0].message.contains(says), "{}", ws[0].message);
    }
    for spec in [
        NodeSpec::column()
            .width(Sizing::Grow(1.0))
            .aspect_ratio(2.0),
        NodeSpec::column()
            .height(Sizing::Fixed(10.0))
            .aspect_ratio(2.0),
    ] {
        assert!(warnings_for(spec).is_empty());
    }
}
