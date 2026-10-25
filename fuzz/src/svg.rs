//! SVG documents (ADR 0063): bytes through [`kui_core::svg::Svg::parse`],
//! then the parsed picture as paths and as a raster at a size byte 0
//! picks, and as an element in a frame under each mode. The properties:
//! nothing panics, the shapes' ops are finite, and a raster is exactly
//! the size it says.

use kui_core::svg::{Svg, SvgFit, SvgMode};
use kui_core::{Color, Core, NodeSpec, Size, SvgOpts, SvgSource};

pub fn run(data: &[u8]) {
    let [size, rest @ ..] = data else {
        return;
    };
    let side = 1.0 + f32::from(size & 63);
    let fit = SvgFit::ALL[usize::from(size >> 6) % 3];
    if let Ok(svg) = Svg::parse(rest) {
        for shape in svg.shapes(side, side, fit, Color::WHITE) {
            assert!(
                shape.path.ops().iter().all(|op| op.is_finite()),
                "a shape with an op that is not finite: {:?}",
                shape.path.ops()
            );
        }
        let (w, h, rgba) = svg.raster(side, side, 2.0, fit, Color::WHITE);
        assert_eq!(rgba.len(), (w * h * 4) as usize);
    }
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(96.0, 96.0), 1.0);
    for mode in SvgMode::ALL {
        ui.svg(
            SvgSource::Data(rest),
            SvgOpts {
                mode,
                fit,
                color: None,
            },
            NodeSpec::default().size(side, side),
        );
    }
    ui.finish();
    let _ = core.output();
}
