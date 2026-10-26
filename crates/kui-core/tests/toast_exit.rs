//! A toast leaving a toaster leaves as one piece: every quad it drew — its
//! hard shadow, its face and edge, its tone bar, its mark, its text, the
//! dismiss button under the pointer — moves by the same offset on every
//! frame of the exit, at the opacity it had, on the frames the view builds
//! and on those the core builds again from the kept tree
//! (`Core::reframe`) alike. Built as Kit's toast is (addons/kit/src/
//! overlays.tl): a row with Ledger's hard shadow, clipping its children,
//! `slide`, an exit that slides it out past the window's right edge and
//! keeps its room, in a toaster floated in the window's corner that slides
//! as it grows and shrinks and leaves the same way.
//!
//! What the report saw was not the toast: it faded as it slid 32 px, and
//! a card's border lying exactly under its right edge showed through and
//! read as that edge left behind. Opaque and sliding out, the toast keeps
//! covering its old place until it has gone past it.
//!
//! A toast that goes by itself gains, on its last frame, the node whose
//! first layout raises its dismiss (Kit's `expire`). In the row's flow it
//! took a gap, and the close button moved 10 px left on the frame the exit
//! holds, then slid: a float, it takes no room, and the × keeps its place
//! on the face from rest through the exit.

use kui_core::{Align, Color, Core, Enter, FloatConfig, NodeSpec, QuadKind, Size, TextStyle};

/// The dismiss button's look, to find it among the quads.
const CLOSE: u32 = 0xf1eee612;

const VIEW: Size = Size {
    w: 1000.0,
    h: 700.0,
};
const INK: u32 = 0xf1eee6ff;
const FACE: u32 = 0x22221fff;
/// Kit's `TOAST_OUT`.
const OUT: f32 = 520.0;

/// The toasts in `toasts`; `due` is the one whose time ran out this frame,
/// carrying the node that raises its dismiss.
fn toaster(core: &mut Core, now: f64, toasts: &[u32], due: Option<u32>) {
    core.set_time(now);
    let mut ui = core.frame(VIEW, 1.0);
    ui.configure_root(NodeSpec::column().fill());
    // The page under it: a card whose right border lies under the toasts'
    // right edges, 16 px in from the window's, as the gallery's do.
    ui.with(NodeSpec::column().fill().pad(16.0), |ui| {
        ui.leaf(
            NodeSpec::column()
                .fill()
                .bg(Color::hex(0x1a1a18ff))
                .border(2.0, Color::hex(INK)),
        );
    });
    if !toasts.is_empty() {
        ui.with_keyed(
            "toaster",
            NodeSpec::column()
                .float(
                    FloatConfig::viewport()
                        .inside(Align::End, Align::End)
                        .offset(-16.0, -16.0)
                        .escaping(),
                )
                .gap(8.0)
                .cross_align(Align::End)
                .exit(Enter::from(OUT, 0.0))
                .slide()
                .transition(180.0),
            |ui| {
                for &id in toasts {
                    toast(ui, id, due == Some(id));
                }
            },
        );
    }
    ui.finish();
}

fn toast(ui: &mut kui_core::Ui<'_>, id: u32, due: bool) {
    ui.with_keyed(
        &format!("toast{id}"),
        NodeSpec::row()
            .min_width(320.0)
            .padding(kui_core::Edges {
                l: 0.0,
                r: 12.0,
                t: 0.0,
                b: 0.0,
            })
            .gap(10.0)
            .cross_align(Align::Center)
            .bg(Color::hex(FACE))
            .radius(2.0)
            .border(2.0, Color::hex(INK))
            .clip()
            // Ledger's hard shadow: the ink, 3 px right and down.
            .shadow_color(Color::hex(INK))
            .shadow_x(3.0)
            .shadow_y(3.0)
            .enter(Enter::from(0.0, 12.0).opacity(0.0))
            .exit(Enter::from(OUT, 0.0).keep_room())
            .slide()
            .transition(180.0),
        |ui| {
            ui.leaf(
                NodeSpec::column()
                    .width(4.0)
                    .grow_height()
                    .bg(Color::hex(0x7fe0a8ff)),
            );
            ui.with(
                NodeSpec::row().grow_width().pad_xy(0.0, 10.0).gap(10.0),
                |ui| {
                    ui.leaf(
                        NodeSpec::column()
                            .size(14.0, 14.0)
                            .bg(Color::hex(0x7fe0a8ff)),
                    );
                    ui.text(&format!("Saved {id}."), TextStyle::new(13.0));
                },
            );
            ui.leaf_keyed(
                "dismiss",
                NodeSpec::row()
                    .size(20.0, 20.0)
                    .radius(2.0)
                    .bg(Color::hex(CLOSE))
                    .on_click("dismiss"),
            );
            // Kit's `expire`: out of the row's flow, so it takes no gap.
            if due {
                ui.leaf_keyed(
                    "expire",
                    NodeSpec::column()
                        .float(FloatConfig::parent())
                        .size(0.0, 0.0)
                        .on_layout("expire"),
                );
            }
        },
    );
}

type Seen = (QuadKind, f32, f32, f32, f32, f32);

/// Every quad drawn in the band `y0..y1` but the page's two, as (kind,
/// x, y, w, h, alpha).
fn band(core: &mut Core, y0: f32, y1: f32) -> Vec<Seen> {
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .filter(|q| q.rect.w < 900.0 && q.rect.y < y1 && q.rect.y + q.rect.h > y0)
        .map(|q| {
            let a = q.color.a.max(q.border_color.a);
            (q.kind, q.rect.x, q.rect.y, q.rect.w, q.rect.h, a)
        })
        .collect()
}

#[test]
fn every_quad_of_a_leaving_toast_moves_together() {
    for groups in [false, true] {
        let mut core = Core::new();
        core.set_opacity_groups(groups);
        // In, and settled.
        for t in [0.0, 0.3, 0.6] {
            toaster(&mut core, t, &[1, 2, 3], None);
        }
        // The middle toast's face, and the band it and its shadow fill.
        let (dl, _) = core.output();
        let mut faces: Vec<_> = dl
            .quads
            .iter()
            .filter(|q| q.kind == QuadKind::Solid && q.color == Color::hex(FACE))
            .map(|q| q.rect)
            .collect();
        faces.sort_by(|a, b| a.y.total_cmp(&b.y));
        assert_eq!(faces.len(), 3, "three toasts");
        let face = faces[1];
        let (y0, y1) = (face.y, face.y + face.h + 3.0);
        let before = band(&mut core, y0, y1);
        assert!(
            before.iter().any(|q| q.0 == QuadKind::Shadow)
                && before.iter().any(|q| q.0 == QuadKind::GlyphMask),
            "its shadow and its text: {before:?}"
        );
        // The close button against the face, at rest.
        let close_on_face = |seen: &[Seen]| {
            let face_x = seen
                .iter()
                .find(|q| q.0 == QuadKind::Solid && (q.3 - face.w).abs() < 0.01)?
                .1;
            let close_x = seen
                .iter()
                .find(|q| {
                    q.0 == QuadKind::Solid && (q.3 - 20.0).abs() < 0.01 && (q.4 - 20.0).abs() < 0.01
                })?
                .1;
            Some(close_x - face_x)
        };
        let rest = close_on_face(&before).expect("its close button");
        let mut closes = vec![rest];

        // Its time runs out: the frame that raises its dismiss is the last
        // one it is placed in, and the one its exit holds.
        toaster(&mut core, 0.9, &[1, 2, 3], Some(2));
        let (dl, _) = core.output();
        assert!(
            dl.quads
                .iter()
                .any(|q| q.rect == face && q.color == Color::hex(FACE)),
            "the face keeps its place on its last frame"
        );
        assert!(
            dl.quads.iter().any(|q| q.color == Color::hex(CLOSE)),
            "the close button is drawn"
        );
        let last = band(&mut core, y0, y1);
        closes.push(close_on_face(&last).expect("its close button"));

        // It leaves; frames built by the view and, where the core can,
        // built again from the kept tree.
        toaster(&mut core, 1.0, &[1, 3], None);
        let mut t = 1.0;
        let mut moved = Vec::new();
        // The exit runs 180 ms; then the room goes and the others move in.
        for step in 0..14 {
            t += 0.012;
            core.set_time(t);
            if step % 2 == 0 || !core.reframe(VIEW, 1.0) {
                toaster(&mut core, t, &[1, 3], None);
            }
            let now = band(&mut core, y0, y1);
            if now.is_empty() {
                continue;
            }
            closes.push(close_on_face(&now).expect("its close button"));
            // Each is one the toast drew before it left, moved right by
            // the exit and by nothing else, as opaque as it was.
            let mut dxs = Vec::new();
            for q in &now {
                let from = before
                    .iter()
                    .filter(|b| b.0 == q.0 && (b.3 - q.3).abs() < 0.01 && (b.4 - q.4).abs() < 0.01)
                    .filter(|b| (b.2 - q.2).abs() < 0.01 && q.1 >= b.1 - 0.01)
                    .filter(|b| (b.5 - q.5).abs() < 0.01)
                    .map(|b| q.1 - b.1)
                    .fold(f32::INFINITY, f32::min);
                assert!(
                    from.is_finite(),
                    "groups {groups}, at {t}: drawn apart from the toast, or faded: {q:?}"
                );
                dxs.push(from);
            }
            let lo = dxs.iter().copied().fold(f32::INFINITY, f32::min);
            let hi = dxs.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            assert!(
                hi - lo < 0.5,
                "groups {groups}, at {t}: its quads moved apart, by {lo}..{hi}: {now:?}"
            );
            // Its old right edge stays under its face until the face has
            // gone past it: nothing of the page shows there as its edge.
            let face_now = now
                .iter()
                .find(|q| q.0 == QuadKind::Solid && (q.3 - face.w).abs() < 0.01)
                .expect("its face");
            if face_now.1 <= face.x + face.w - 2.0 {
                assert!(face_now.1 + face_now.3 >= face.x + face.w);
            }
            moved.push(lo);
        }
        assert!(moved.len() >= 5, "the exit was drawn: {moved:?}");
        assert!(
            moved.windows(2).all(|w| w[1] >= w[0] - 0.01) && moved[moved.len() - 1] > 100.0,
            "it slides out to the right: {moved:?}"
        );
        // The × on the face, from rest through its last frame and every
        // frame of the exit, the first included.
        assert!(
            closes.iter().all(|dx| (dx - rest).abs() < 0.01),
            "groups {groups}: the close button against the face: {closes:?}"
        );
    }
}
