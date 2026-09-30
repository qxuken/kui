//! `Core::set_overscan`: what a clip hides is painted anyway, this far
//! past its edge, so a driver that moves quads between two frames has
//! something to bring into view. Paint only — nothing out there can be
//! hit — and clipped as before, so a backend draws the same pixels.

use kui_core::{Core, NodeSpec, Quad, QuadKind, Rect, Size, TextStyle, Value};

/// A 100 px scroller at the top of the window, holding twenty 20 px rows
/// of one text each, every row clickable.
fn frame(core: &mut Core) {
    let mut ui = core.frame(Size::new(200.0, 400.0), 1.0);
    ui.with(NodeSpec::column().fill(), |ui| {
        ui.with_keyed(
            "list",
            NodeSpec::column().width(200.0).height(100.0).scroll_y(),
            |ui| {
                for i in 0..20u64 {
                    ui.text_in_indexed(
                        i,
                        NodeSpec::row()
                            .width(200.0)
                            .height(20.0)
                            .on_click(Value::Int(i as i64)),
                        "row",
                        TextStyle::new(12.0),
                    );
                }
            },
        );
    });
    ui.finish();
}

fn glyphs(core: &mut Core) -> Vec<(Quad, Rect)> {
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .filter(|q| q.kind != QuadKind::Solid)
        .map(|q| (*q, dl.clip_of(q).rect))
        .collect()
}

#[test]
fn rows_within_the_overscan_are_painted_under_the_clip_that_hides_them() {
    let mut core = Core::new();
    frame(&mut core);
    let plain = glyphs(&mut core);
    let hits = core.interaction.hits().len();
    assert!(!plain.is_empty(), "the visible rows draw their text");
    assert!(
        plain.iter().all(|(q, _)| q.rect.y < 100.0),
        "without an overscan nothing past the clip is in the list"
    );

    core.set_overscan(40.0);
    frame(&mut core);
    let over = glyphs(&mut core);
    // Two more rows of three glyphs: the ones from 100 to 140.
    assert_eq!(over.len(), plain.len() + 6);
    let beyond: Vec<_> = over.iter().filter(|(q, _)| q.rect.y >= 100.0).collect();
    assert_eq!(beyond.len(), 6);
    for (q, clip) in &beyond {
        assert!(q.rect.y < 140.0, "and no further than the overscan");
        assert_eq!(
            (clip.y, clip.h),
            (0.0, 100.0),
            "an overscanned quad names the scroller's clip, so it is not drawn"
        );
    }
    assert_eq!(
        core.interaction.hits().len(),
        hits,
        "and nothing out there can be hit"
    );

    core.set_overscan(0.0);
    frame(&mut core);
    assert_eq!(
        glyphs(&mut core).len(),
        plain.len(),
        "off again, gone again"
    );
}
