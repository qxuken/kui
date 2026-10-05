//! The half of the gradient contract shader validation cannot see
//! (`docs/adr/0042-a-gradient-is-an-image-the-core-paints.md`, backlog
//! V9): the core rasterizes a gradient into a small slot of the atlas and
//! emits an `Image` quad stretched over the box, and the shader's
//! `kind == 3u` branch samples the atlas linearly across it
//! (`src/shader.wgsl`). Whether a strip of 256 texels, or a square of
//! 128, stretched to a box of any size is *the gradient* — at its edges,
//! where the sampler reads past the rect it was given, on a diagonal,
//! round a centre — is arithmetic no compile checks.
//!
//! This mirrors that branch on the CPU over the atlas and the quads a
//! core actually emitted, and compares every pixel with the gradient
//! computed at that pixel. The numbers it prints are the ADR's
//! measurements; the bounds are what it holds them to.

use kui_core::{Color, Core, Gradient, GradientStop, NodeSpec, Quad, QuadKind, Side, Size, Vec2};

/// The atlas sampled as the linear sampler samples it: texel centres at
/// the half, the four around `(u, v)` mixed, and nothing clamped to the
/// slot — which is the point.
fn sample(core: &Core, u: f32, v: f32) -> [f32; 4] {
    let size = core.atlas.size as i64;
    let texel = |x: i64, y: i64| -> [f32; 4] {
        let (x, y) = (x.clamp(0, size - 1), y.clamp(0, size - 1));
        let at = ((y * size + x) * 4) as usize;
        std::array::from_fn(|k| f32::from(core.atlas.pixels[at + k]) / 255.0)
    };
    let (fu, fv) = (u - 0.5, v - 0.5);
    let (x0, y0) = (fu.floor(), fv.floor());
    let (wx, wy) = (fu - x0, fv - y0);
    let (x0, y0) = (x0 as i64, y0 as i64);
    let (a, b, c, d) = (
        texel(x0, y0),
        texel(x0 + 1, y0),
        texel(x0, y0 + 1),
        texel(x0 + 1, y0 + 1),
    );
    std::array::from_fn(|k| {
        (a[k] * (1.0 - wx) + b[k] * wx) * (1.0 - wy) + (c[k] * (1.0 - wx) + d[k] * wx) * wy
    })
}

/// The image branch of `shade` at a framebuffer pixel's centre: `uv`
/// interpolated across the quad, the texel there, tinted.
fn shade(core: &Core, q: &Quad, x: f32, y: f32) -> [f32; 4] {
    let (fx, fy) = ((x - q.rect.x) / q.rect.w, (y - q.rect.y) / q.rect.h);
    let t = sample(
        core,
        q.uv[0] as f32 + fx * q.uv[2] as f32,
        q.uv[1] as f32 + fy * q.uv[3] as f32,
    );
    [
        t[0] * q.color.r,
        t[1] * q.color.g,
        t[2] * q.color.b,
        t[3] * q.color.a,
    ]
}

/// One `w × h` box with `g` at (10, 10), beside other things in the
/// atlas so a slot that bled would bleed into something: a second
/// gradient before it and a third after.
fn boxed(g: &Gradient, w: f32, h: f32) -> (Core, Quad) {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(w + 20.0, h + 80.0), 1.0);
    let noise = |turns: f32| {
        NodeSpec::column()
            .size(8.0, 8.0)
            .gradient(Gradient::angle(turns, [Color::WHITE, Color::BLACK]))
    };
    ui.leaf(noise(0.0));
    ui.leaf(noise(0.3));
    ui.leaf(NodeSpec::column().size(w, h).gradient(g.clone()));
    ui.leaf(noise(0.25));
    ui.finish();
    let q = core
        .output()
        .0
        .quads
        .iter()
        .filter(|q| q.kind == QuadKind::Image)
        .nth(2)
        .copied()
        .expect("the box's image quad");
    assert_eq!((q.rect.w, q.rect.h), (w, h));
    (core, q)
}

/// The largest difference, in 8-bit levels, between what the quad draws
/// and the gradient computed at each pixel's centre — over every pixel
/// of the box, on any channel, alpha included.
fn worst(g: &Gradient, w: f32, h: f32) -> f32 {
    let (core, q) = boxed(g, w, h);
    let mut worst = 0.0f32;
    for py in 0..h as u32 {
        for px in 0..w as u32 {
            let (x, y) = (px as f32 + 0.5, py as f32 + 0.5);
            let got = shade(&core, &q, q.rect.x + x, q.rect.y + y);
            let c = g.color_in(x / w, y / h);
            let want = [c.r, c.g, c.b, c.a];
            for k in 0..4 {
                worst = worst.max((got[k] - want[k]).abs() * 255.0);
            }
        }
    }
    worst
}

const BLUE: u32 = 0x7f9cf5ff;
const PINK: u32 = 0xe07a8aff;
const DARK: u32 = 0x14161eff;

fn two(a: u32, b: u32) -> [Color; 2] {
    [Color::hex(a), Color::hex(b)]
}

/// A strip stretched to a thousand pixels is the gradient at every one
/// of them — the first and last columns, and the top and bottom rows a
/// strip one texel high would fade out on, included.
#[test]
fn a_strip_stretched_over_a_box_is_the_gradient_to_its_edges() {
    for side in [Side::Right, Side::Left, Side::Bottom, Side::Top] {
        let g = Gradient::to(side, two(BLUE, PINK));
        let (w, h) = match side {
            Side::Right | Side::Left => (1000.0, 40.0),
            _ => (40.0, 1000.0),
        };
        let d = worst(&g, w, h);
        println!("strip {side:?} over {w}x{h}: {d:.2} levels");
        assert!(d <= 1.0, "{side:?}: {d} levels");
    }
    // The full range, where a level is a 255th of the box.
    let g = Gradient::to(Side::Right, [Color::BLACK, Color::WHITE]);
    let d = worst(&g, 1000.0, 40.0);
    println!("strip black to white over 1000x40: {d:.2} levels");
    assert!(d <= 1.0, "{d} levels");
}

/// The measurement the square's size hangs on: a corner, an angle and a
/// radial on a box three times as wide as it is tall, 128 texels a side
/// stretched to 600 × 200, against the mix computed per pixel. A smooth
/// ramp is held to two levels.
#[test]
fn the_square_stretched_to_a_3_to_1_box_is_within_two_levels() {
    let cases: [(&str, Gradient); 5] = [
        ("corner", Gradient::to(Side::BottomRight, two(BLUE, PINK))),
        ("angle 0.07", Gradient::angle(0.07, two(BLUE, PINK))),
        (
            "corner, full range",
            Gradient::to(Side::TopRight, [Color::BLACK, Color::WHITE]),
        ),
        ("radial", Gradient::radial(two(BLUE, DARK))),
        (
            "radial from the top edge",
            Gradient::radial_at(Vec2::new(0.5, 0.0), two(PINK, DARK)),
        ),
    ];
    for (name, g) in cases {
        let d = worst(&g, 600.0, 200.0);
        println!("square, {name}, over 600x200: {d:.2} levels");
        assert!(d <= 2.0, "{name}: {d} levels");
    }
}

/// Three stops put a kink in the ramp, which a raster rounds off over a
/// texel: the worst pixel sits on the kink.
#[test]
fn a_middle_stop_s_kink_costs_a_few_levels_at_the_kink() {
    let stops = [Color::hex(BLUE), Color::hex(0xffe066ff), Color::hex(PINK)];
    let strip = worst(&Gradient::to(Side::Right, stops), 1000.0, 40.0);
    let square = worst(&Gradient::to(Side::BottomRight, stops), 600.0, 200.0);
    println!("three stops: strip {strip:.2} levels, square {square:.2} levels");
    assert!(strip <= 2.0, "{strip}");
    assert!(square <= 4.0, "{square}");
}

/// A hard stop is as soft as the raster stretched: how many pixels of a
/// thousand are neither colour.
#[test]
fn a_hard_stop_on_a_strip_is_a_few_pixels_wide() {
    let g = Gradient::to(
        Side::Right,
        [
            GradientStop::from((Color::BLACK, 0.5)),
            GradientStop::from((Color::WHITE, 0.5)),
        ],
    );
    let (core, q) = boxed(&g, 1000.0, 20.0);
    let between = (0..1000)
        .filter(|&px| {
            let v = shade(&core, &q, q.rect.x + px as f32 + 0.5, q.rect.y + 10.0)[0];
            v > 0.02 && v < 0.98
        })
        .count();
    println!("a hard stop over 1000 px: {between} px between the two colours");
    // 1000 / 256 is 3.9 px a texel, and the ramp is one texel wide.
    assert!((1..=5).contains(&between), "{between}");
}

/// Red fading out is red all the way, at the alpha the fade is at — not
/// the darkened red a straight mix with transparent black would draw.
#[test]
fn a_fade_to_transparent_keeps_its_colour_across_the_box() {
    let g = Gradient::to(Side::Right, [Color::hex(0xff0000ff), Color::TRANSPARENT]);
    let (core, q) = boxed(&g, 400.0, 20.0);
    for px in [10.0, 100.0, 200.0, 300.0, 380.0] {
        let c = shade(&core, &q, q.rect.x + px + 0.5, q.rect.y + 10.0);
        let want = 1.0 - (px + 0.5) / 400.0;
        assert!(c[0] > 0.99 && c[1] < 0.01, "at {px}: {c:?}");
        assert!(
            (c[3] - want).abs() < 0.01,
            "alpha at {px}: {} vs {want}",
            c[3]
        );
    }
}
