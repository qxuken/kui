//! Paths: SVG's `d` through [`Path::parse`] (the one parser every binding
//! goes through) or the flat wire form through [`Path::from_floats`] (C and
//! Node), then everything the core does with the ops — flattening for the
//! hit outline, rasterizing the mask, and a frame that declares the path
//! as a node, filled or stroked, turned or not, at a scale.
//!
//! The input is a four-byte header and the ops, so a seed is a `d` string
//! with four bytes in front of it:
//!
//! - byte 0: bit 0 the ops are the wire's little-endian `f32`s rather
//!   than `d` text, bit 1 the even-odd rule, bit 2 stroked, bit 3 turned;
//! - byte 1: the stroke's width in eighths of a px;
//! - byte 2: the turn in sixteenths, signed;
//! - byte 3: the scale, an index into [`SCALES`].

use kui_core::path::{self, MaskPaint};
use kui_core::{Color, Core, FillRule, NodeSpec, Path, Size, Stroke, Vec2};

/// The scales a window is drawn at: a fraction, the common ones, and a
/// zoomed-in display.
const SCALES: &[f32] = &[1.0, 0.5, 1.25, 1.5, 2.0, 3.0, 4.0];

struct Input<'a> {
    floats: bool,
    evenodd: bool,
    stroke: Option<u8>,
    turns: Option<i8>,
    scale: u8,
    ops: &'a [u8],
}

enum Source {
    D(String),
    Floats(Vec<f32>),
}

pub fn run(data: &[u8]) {
    let [flags, width, turn, scale, ops @ ..] = data else {
        return;
    };
    let input = Input {
        floats: flags & 1 != 0,
        evenodd: flags & 2 != 0,
        stroke: (flags & 4 != 0).then_some(*width),
        turns: (flags & 8 != 0).then_some(*turn as i8),
        scale: *scale,
        ops,
    };
    let source = if input.floats {
        Source::Floats(
            input
                .ops
                .as_chunks::<4>()
                .0
                .iter()
                .map(|b| f32::from_le_bytes(*b))
                .collect(),
        )
    } else {
        match std::str::from_utf8(input.ops) {
            Ok(d) => Source::D(d.to_string()),
            Err(_) => return,
        }
    };
    let path = match &source {
        Source::D(d) => match Path::parse(d) {
            Ok(p) => {
                // The wire form of what parsed reads back to the same ops.
                let floats = p.to_floats();
                let back = Path::from_floats(&floats).expect("the wire form of parsed ops");
                assert_eq!(bits(&back.to_floats()), bits(&floats), "{d:?}");
                p
            }
            Err(_) => return,
        },
        Source::Floats(f) => match Path::from_floats(f) {
            Ok(p) => p,
            Err(_) => return,
        },
    };
    let ops = path.ops();

    let mut fill = Vec::new();
    path::flatten(ops, &mut fill);
    let mut stroke = Vec::new();
    path::flatten_stroke(ops, &mut stroke);
    if let Some(r) = path::bounds(&fill) {
        for p in [
            Vec2::new(r.x, r.y),
            r.center(),
            Vec2::new(r.x + r.w, r.y + r.h),
        ] {
            let _ = path::in_path(p, &fill, FillRule::NonZero);
            let _ = path::in_path(p, &fill, FillRule::EvenOdd);
        }
    }
    let _ = path::hash_ops(ops);
    let scale = SCALES[input.scale as usize % SCALES.len()];
    for paint in [
        MaskPaint::Fill(FillRule::NonZero),
        MaskPaint::Stroke(1.5 * scale),
    ] {
        let mask = path::rasterize(ops, scale, (1, 3), 48, 32, paint);
        assert_eq!(mask.len(), 48 * 32);
    }

    // The node: the core sizes its float, picks atlas or texture, hits it.
    let mut p = path.fill_rule(if input.evenodd {
        FillRule::EvenOdd
    } else {
        FillRule::NonZero
    });
    if let Some(w) = input.stroke {
        p = p.stroked(Stroke::new(f32::from(w) / 8.0, Color::hex(0xf5d67fff)));
    }
    if let Some(t) = input.turns {
        p = p.rotated(f32::from(t) / 16.0);
    }
    let mut core = Core::new();
    for _ in 0..2 {
        let mut ui = core.frame(Size::new(320.0, 240.0), scale);
        ui.configure_root(NodeSpec::column().fill());
        ui.path(
            &p,
            NodeSpec::column()
                .bg(Color::hex(0x3a7bd5ff))
                .on_click("hit"),
        );
        ui.finish();
        let _ = core.output();
    }
    core.handle_input(kui_core::InputEvent::CursorMoved(Vec2::new(40.0, 40.0)));
    core.handle_input(kui_core::InputEvent::mouse_down(1));
    core.handle_input(kui_core::InputEvent::mouse_up());
}

fn bits(f: &[f32]) -> Vec<u32> {
    f.iter().map(|v| v.to_bits()).collect()
}
