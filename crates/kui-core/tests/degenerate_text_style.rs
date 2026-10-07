//! A text style whose size or line height is no size at all — 0, under
//! half a pixel, negative, NaN — shapes at a pixel instead of reaching
//! the shaper as it is: cosmic-text asserts a line height is not 0, which
//! aborted a Node or Lua process (`lineHeight = 0`, `size = 0.3`), and a
//! negative one spun its layout until memory ran out. C's door had always
//! read those as unset; the other bindings hand the number through.

use kui_core::{Cell, CellGrid, Core, EditOptions, NodeSpec, Size, TextStyle};

fn styles() -> Vec<TextStyle> {
    let mut out = Vec::new();
    for v in [0.0, 0.3, -1.0, -0.5, f32::NAN] {
        out.push(TextStyle::new(v));
        out.push(TextStyle::new(14.0).line_height(v));
    }
    out
}

#[test]
fn a_size_or_line_height_of_nothing_lays_out_and_draws() {
    let mut core = Core::new();
    let cells = vec![Cell::new('x', 0xffffffff, 0); 4];
    for style in styles() {
        let mut ui = core.frame(Size::new(200.0, 100.0), 1.0);
        ui.text("hi there", style);
        ui.text_edit(
            "e",
            "edit me",
            &EditOptions {
                style,
                ..Default::default()
            },
            NodeSpec::column().width(100.0),
        );
        let grid = CellGrid {
            rows: 2,
            cols: 2,
            cells: &cells,
            style,
            cursor: None,
            origin_line: 0,
        };
        ui.cells_keyed("g", &grid, NodeSpec::column());
        ui.finish();
        let m = core.measure_text("hi", &style, Some(50.0));
        assert!(
            m.width.is_finite() && m.height.is_finite(),
            "{style:?}: {m:?}"
        );
    }
}
