//! Backlog W7: glyphs are placed at whole physical pixels and their box was
//! not, so anything *moving* — a slide, an entrance, an exit, a scroll —
//! made its text wobble ±0.5 px inside its own background for as long as
//! the motion lasted. Every displacement is snapped to whole physical
//! pixels now, so the two move together. Measured the way the bug was: the
//! gap between a card's left edge and its first glyph, frame by frame.

use kui_core::{
    Color, Core, Easing, Enter, Key, NodeSpec, Quad, QuadKind, Size, Sizing, TextStyle, Transition,
    Vec2,
};

fn is_glyph(q: &Quad) -> bool {
    matches!(q.kind, QuadKind::GlyphMask | QuadKind::GlyphSubpixel)
}

/// The first background quad's physical origin and the first glyph's, for
/// whatever the last frame drew.
fn box_and_glyph(core: &mut Core) -> (Vec2, Vec2) {
    let (dl, _) = core.output();
    let bg = dl
        .quads
        .iter()
        .find(|q| q.kind == QuadKind::Solid)
        .expect("the card's background")
        .rect;
    let glyph = dl.quads.iter().find(|q| is_glyph(q)).expect("a glyph").rect;
    (Vec2::new(bg.x, bg.y), Vec2::new(glyph.x, glyph.y))
}

const SLIDE: f64 = 0.26;

/// One frame of a card entering from 100 px to the left, at scale 1.5 —
/// the case W7 was measured on. `pad_l` shifts where it comes to rest, so
/// the same fixture covers a whole-pixel target and a fractional one.
fn slide_frame(core: &mut Core, t: f64, pad_l: f32) -> (Vec2, Vec2) {
    core.set_time(t);
    let mut ui = core.frame(Size::new(400.0, 100.0), 1.5);
    ui.configure_root(NodeSpec::row().fill().padding(kui_core::Edges {
        l: pad_l,
        ..kui_core::Edges::all(0.0)
    }));
    let spec = NodeSpec::column()
        .width(Sizing::Fixed(200.0))
        .height(Sizing::Fixed(60.0))
        .pad(14.0)
        .bg(Color::WHITE)
        .transition_with(Transition::ms(SLIDE as f32 * 1000.0).easing(Easing::Linear))
        .enter(Enter::from(-100.0, 0.0));
    ui.with_keyed("card", spec, |ui| {
        ui.text("Hi", TextStyle::new(14.0));
    });
    ui.finish();
    box_and_glyph(core)
}

#[test]
fn text_does_not_wobble_inside_a_sliding_box() {
    let mut core = Core::new();
    let (start, glyph) = slide_frame(&mut core, 0.0, 0.0);
    let gap = glyph.x - start.x;
    let mut xs = vec![start.x];
    for i in 1..=13 {
        let (b, g) = slide_frame(&mut core, i as f64 * SLIDE / 13.0, 0.0);
        assert_eq!(g.x - b.x, gap, "the glyph moved inside the card, frame {i}");
        xs.push(b.x);
    }
    assert!(
        xs.windows(2).any(|w| w[0] != w[1]),
        "the card should have been moving: {xs:?}"
    );
    // This card's resting place is a whole physical pixel, so every frame
    // of the slide is one too — a whole-pixel offset from a whole-pixel
    // target. `a_fractional_target_stays_where_layout_put_it` is the other
    // half: what is snapped is the offset, not the position.
    for x in &xs {
        assert_eq!(*x, x.round(), "a moving card sits on a physical pixel: {x}");
    }
    assert_eq!(*xs.last().unwrap(), 0.0, "and lands exactly on its target");
}

#[test]
fn a_fractional_target_stays_where_layout_put_it() {
    // 7/1.5 px of padding is 10.5 physical: the card rests off the pixel
    // grid and must stay there — snapping the *position* would move a
    // still card by half a pixel the moment it stopped animating.
    let mut core = Core::new();
    let (start, glyph) = slide_frame(&mut core, 0.0, 7.0);
    let gap = glyph.x - start.x;
    for i in 1..=13 {
        let (b, g) = slide_frame(&mut core, i as f64 * SLIDE / 13.0, 7.0);
        assert_eq!(g.x - b.x, gap, "the glyph moved inside the card, frame {i}");
    }
    let (rest, _) = slide_frame(&mut core, 1.0, 7.0);
    assert_eq!(rest.x, 10.5, "settled where layout put it, off the grid");
}

/// One frame of a list scrolled `off` logical px, at scale 1.5.
fn scroll_frame(core: &mut Core, off: f32) -> (Vec2, Vec2) {
    core.set_scroll(Key::ROOT.str("list"), Vec2::new(0.0, off));
    let mut ui = core.frame(Size::new(400.0, 100.0), 1.5);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed("list", NodeSpec::column().fill().scroll_y(), |ui| {
        for i in 0..8 {
            let row = NodeSpec::column()
                .width(Sizing::Fixed(200.0))
                .height(Sizing::Fixed(30.0))
                .bg(Color::WHITE);
            ui.with_keyed(&format!("row{i}"), row, |ui| {
                ui.text("Hi", TextStyle::new(14.0));
            });
        }
    });
    ui.finish();
    box_and_glyph(core)
}

#[test]
fn text_does_not_wobble_inside_a_scrolling_row() {
    let mut core = Core::new();
    let (top, glyph) = scroll_frame(&mut core, 0.0);
    let gap = glyph.y - top.y;
    let mut ys = vec![top.y];
    // Fractions of a logical pixel, which a trackpad and a scrollbar drag
    // both produce and 150% turns into fractions of a physical one.
    for off in [0.1, 0.3, 0.7, 1.1, 1.9, 4.4] {
        let (b, g) = scroll_frame(&mut core, off);
        assert_eq!(g.y - b.y, gap, "the glyph moved inside its row at {off}");
        ys.push(b.y);
        assert_eq!(b.y, b.y.round(), "a scrolled row sits on a pixel: {}", b.y);
    }
    assert!(ys.windows(2).any(|w| w[0] != w[1]), "nothing scrolled");
}

#[test]
fn text_does_not_wobble_inside_a_departing_box() {
    // A ghost is mostly text and moves for its whole life, so it snaps the
    // same way a live node does.
    let mut core = Core::new();
    let frame = |core: &mut Core, t: f64, present: bool| {
        core.set_time(t);
        let mut ui = core.frame(Size::new(400.0, 100.0), 1.5);
        ui.configure_root(NodeSpec::row().fill());
        if present {
            let spec = NodeSpec::column()
                .width(Sizing::Fixed(200.0))
                .height(Sizing::Fixed(60.0))
                .pad(14.0)
                .bg(Color::WHITE)
                .transition_with(Transition::ms(260.0).easing(Easing::Linear))
                .exit(Enter::from(101.0, 0.0));
            ui.with_keyed("card", spec, |ui| {
                ui.text("Hi", TextStyle::new(14.0));
            });
        }
        ui.finish();
    };
    frame(&mut core, 0.0, true);
    let (b, g) = box_and_glyph(&mut core);
    let gap = g.x - b.x;
    let mut xs = vec![];
    for i in 1..=12 {
        frame(&mut core, i as f64 * 0.02, false);
        let (b, g) = box_and_glyph(&mut core);
        assert_eq!(
            g.x - b.x,
            gap,
            "the glyph moved inside the ghost, frame {i}"
        );
        assert_eq!(
            b.x,
            b.x.round(),
            "a departing card sits on a pixel: {}",
            b.x
        );
        xs.push(b.x);
    }
    assert!(xs.windows(2).any(|w| w[0] != w[1]), "nothing departed");
}
