//! The `svg` element through the core (ADR 0063): an icon drawn as paths
//! in its own size or a size the view gives, `currentColor` painted the
//! element's colour, a gradient drawn as one shared raster, a box that
//! grows drawn at its laid-out size a frame later, a file that redraws
//! when it changes, and the warnings for what cannot draw.

use kui_core::svg::Svg;
use kui_core::{Color, Core, NodeSpec, QuadKind, Role, Size, SvgFit, SvgMode, SvgOpts, SvgSource};

const VIEW: Size = Size { w: 300.0, h: 200.0 };

const ICON: &[u8] =
    br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"
  fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/></svg>"#;

const GRADIENT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20">
  <defs><linearGradient id="g"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient></defs>
  <rect width="40" height="20" fill="url(#g)"/></svg>"#;

fn frame(core: &mut Core, build: impl FnOnce(&mut kui_core::Ui<'_>)) {
    let mut ui = core.frame(VIEW, 2.0);
    build(&mut ui);
    ui.finish();
}

fn image_node(core: &mut Core, name: &str) -> kui_core::AccessNode {
    core.access_tree()
        .nodes
        .iter()
        .find(|n| n.name.as_deref() == Some(name))
        .cloned()
        .unwrap_or_else(|| panic!("no node named {name}"))
}

fn quads(core: &mut Core, kind: QuadKind) -> Vec<kui_core::Quad> {
    core.output()
        .0
        .quads
        .iter()
        .filter(|q| q.kind == kind)
        .copied()
        .collect()
}

fn codes(core: &mut Core) -> Vec<&'static str> {
    core.take_warnings().iter().map(|w| w.code).collect()
}

#[test]
fn an_icon_is_paths_in_its_own_size_painted_its_colour() {
    let mut core = Core::new();
    let ink = Color::rgba8(200, 30, 40, 255);
    frame(&mut core, |ui| {
        ui.svg(
            SvgSource::Data(ICON),
            SvgOpts {
                color: Some(ink),
                ..SvgOpts::default()
            },
            NodeSpec::default().label("Search"),
        );
    });
    let node = image_node(&mut core, "Search");
    assert_eq!(node.role, Role::Image);
    assert_eq!((node.rect.w, node.rect.h), (24.0, 24.0));
    let masks = quads(&mut core, QuadKind::GlyphMask);
    assert_eq!(masks.len(), 2, "a circle and a stroke: {masks:?}");
    assert!(masks.iter().all(|q| q.color == ink), "{masks:?}");
    assert_eq!(core.svg_raster_count(), 0);
    assert!(codes(&mut core).is_empty());
}

#[test]
fn one_axis_given_keeps_the_aspect() {
    let mut core = Core::new();
    frame(&mut core, |ui| {
        ui.svg(
            SvgSource::Data(GRADIENT),
            SvgOpts::default(),
            NodeSpec::default().width(80.0).label("Bar"),
        );
    });
    let node = image_node(&mut core, "Bar");
    assert_eq!((node.rect.w, node.rect.h), (80.0, 40.0));
}

#[test]
fn a_gradient_is_one_raster_shared_by_every_element_drawing_it() {
    let mut core = Core::new();
    frame(&mut core, |ui| {
        for name in ["A", "B"] {
            ui.svg(
                SvgSource::Data(GRADIENT),
                SvgOpts::default(),
                NodeSpec::default().label(name),
            );
        }
    });
    assert_eq!(core.svg_raster_count(), 1);
    assert_eq!(quads(&mut core, QuadKind::GlyphMask).len(), 0);
    // Forced to paths: the gradient's fill is what a path cannot draw.
    frame(&mut core, |ui| {
        ui.svg(
            SvgSource::Data(GRADIENT),
            SvgOpts {
                mode: SvgMode::Paths,
                ..SvgOpts::default()
            },
            NodeSpec::default().label("A"),
        );
    });
    assert!(codes(&mut core).contains(&"svg-approximated"));
}

#[test]
fn an_icon_forced_to_an_image_rasterizes_at_the_frames_scale() {
    let mut core = Core::new();
    frame(&mut core, |ui| {
        ui.svg(
            SvgSource::Data(ICON),
            SvgOpts {
                mode: SvgMode::Image,
                fit: SvgFit::Fill,
                ..SvgOpts::default()
            },
            NodeSpec::default().size(32.0, 32.0).label("Search"),
        );
    });
    assert_eq!(core.svg_raster_count(), 1);
    assert_eq!(quads(&mut core, QuadKind::GlyphMask).len(), 0);
}

#[test]
fn a_growing_box_draws_at_its_laid_out_size_the_next_frame() {
    let mut core = Core::new();
    let view = |ui: &mut kui_core::Ui<'_>| {
        ui.with(NodeSpec::row().size(120.0, 60.0), |ui| {
            ui.svg(
                SvgSource::Data(ICON),
                SvgOpts::default(),
                NodeSpec::default()
                    .width(kui_core::Sizing::Grow(1.0))
                    .height(60.0)
                    .label("Search"),
            );
        });
    };
    frame(&mut core, view);
    // Drawn at its own size the first time, and the frame that draws it
    // at the laid-out size is owed.
    assert!(core.owed().requested);
    let first: Vec<_> = quads(&mut core, QuadKind::GlyphMask);
    frame(&mut core, view);
    let second: Vec<_> = quads(&mut core, QuadKind::GlyphMask);
    assert!(!core.owed().requested, "settled");
    // Contained: the same scale in the wider box, centred — 30 px
    // further right, 60 physical px at scale 2.
    assert_eq!(second[0].rect.w, first[0].rect.w);
    assert_eq!(
        second[0].rect.x - first[0].rect.x,
        60.0,
        "{first:?} → {second:?}"
    );
    assert_eq!(image_node(&mut core, "Search").rect.w, 120.0);
}

#[test]
fn a_file_redraws_when_it_changes_and_warns_while_it_cannot_be_read() {
    let dir = std::env::temp_dir().join(format!("kui-svg-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("icon.svg");
    let path_str = path.to_str().unwrap().to_string();
    let mut core = Core::new();
    let view = |ui: &mut kui_core::Ui<'_>| {
        ui.svg(
            SvgSource::File(&path_str),
            SvgOpts::default(),
            NodeSpec::default().label("Icon"),
        );
    };
    frame(&mut core, view);
    assert!(codes(&mut core).contains(&"svg-unreadable"));
    std::fs::write(&path, ICON).unwrap();
    frame(&mut core, view);
    assert_eq!(image_node(&mut core, "Icon").rect.w, 24.0);
    std::fs::write(&path, GRADIENT).unwrap();
    frame(&mut core, view);
    assert_eq!(image_node(&mut core, "Icon").rect.w, 40.0);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_document_that_does_not_parse_draws_an_empty_box_and_says_so() {
    let mut core = Core::new();
    frame(&mut core, |ui| {
        ui.svg(
            SvgSource::Data(b"<svg"),
            SvgOpts::default(),
            NodeSpec::default().size(10.0, 10.0).label("Broken"),
        );
    });
    assert_eq!(codes(&mut core), ["svg-malformed"]);
    assert!(Svg::parse(b"<svg").is_err());
}

#[test]
fn an_unlabelled_svg_is_an_unlabelled_image() {
    let mut core = Core::new();
    frame(&mut core, |ui| {
        ui.svg(
            SvgSource::Data(ICON),
            SvgOpts::default(),
            NodeSpec::default(),
        );
        ui.svg(
            SvgSource::Data(ICON),
            SvgOpts::default(),
            NodeSpec::default().role(Role::None),
        );
    });
    let image_warnings = codes(&mut core)
        .into_iter()
        .filter(|c| *c == "image-without-label")
        .count();
    assert_eq!(image_warnings, 1);
}
