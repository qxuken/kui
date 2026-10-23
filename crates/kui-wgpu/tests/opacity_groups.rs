//! A faded subtree drawn as one picture (backlog V11, ADR 0056), in
//! pixels: what a group draws is what fading the whole subtree at once
//! draws — the frame with the subtree opaque and the frame without it,
//! mixed by the opacity, pixel by pixel. The fade per quad it replaces is
//! measured against the same reference, so the test can tell the two
//! apart. On the machine's GPU; skipped, saying so, where there is no
//! adapter.
//!
//! Subpixel text is off throughout: a group draws its glyphs grayscale
//! (a picture over nothing has no colour behind a glyph to split its
//! coverage against), and the reference's opaque frame would not.

use kui_core::{Color, Core, NodeSpec, Size, TextStyle};
use kui_wgpu::{Gpu, Renderer, wgpu};

fn gpu() -> Option<Gpu> {
    match pollster::block_on(Gpu::windowless()) {
        Ok(g) => {
            let name = g.adapter().get_info().name;
            if let Ok(want) = std::env::var("KUI_REQUIRE_GPU")
                && !want.is_empty()
                && want != "1"
            {
                assert!(
                    name.contains(&want),
                    "KUI_REQUIRE_GPU={want}, but the adapter is {name}"
                );
            }
            Some(g)
        }
        Err(e) => {
            if std::env::var_os("KUI_REQUIRE_GPU").is_some_and(|v| !v.is_empty()) {
                panic!("KUI_REQUIRE_GPU is set, but no adapter ({e})");
            }
            eprintln!("skipped: no adapter ({e})");
            None
        }
    }
}

const VIEW: Size = Size { w: 120.0, h: 70.0 };

/// The page and a dark card with a white label and a red dot on it, the
/// card at `card` and, when `panel` is some, inside a light panel at that
/// opacity. (A float under the card would be outside its group, faded
/// per quad by design, so the dot is in flow.) `turn` turns the card; `cut` puts it in
/// a parent that clips it.
#[derive(Clone, Copy)]
struct Scene {
    card: f32,
    panel: Option<f32>,
    turn: f32,
    cut: bool,
}

impl Scene {
    fn card(card: f32) -> Self {
        Self {
            card,
            panel: None,
            turn: 0.0,
            cut: false,
        }
    }

    fn build(self, core: &mut Core) {
        let mut ui = core.frame(VIEW, 2.0);
        ui.configure_root(
            NodeSpec::column()
                .fill()
                .pad(10.0)
                .bg(Color::hex(0xf0e8d8ff)),
        );
        let card = |ui: &mut kui_core::Ui<'_>| {
            let mut spec = NodeSpec::row()
                .gap(6.0)
                .size(70.0, 34.0)
                .pad(8.0)
                .radius(6.0)
                .bg(Color::hex(0x202020ff))
                .opacity(self.card);
            if self.turn != 0.0 {
                spec = spec.rotate(self.turn);
            }
            ui.with_keyed("card", spec, |ui| {
                ui.text("Saved", TextStyle::new(13.0).color(Color::WHITE));
                ui.leaf_keyed(
                    "dot",
                    NodeSpec::column()
                        .size(16.0, 16.0)
                        .radius(8.0)
                        .bg(Color::hex(0xd03030ff)),
                );
            });
        };
        let holder = |ui: &mut kui_core::Ui<'_>| {
            if self.cut {
                ui.with_keyed("cut", NodeSpec::column().size(60.0, 30.0).clip(), card);
            } else {
                card(ui);
            }
        };
        match self.panel {
            Some(o) => {
                ui.with_keyed(
                    "panel",
                    NodeSpec::column()
                        .pad(6.0)
                        .bg(Color::hex(0xc8d8f0ff))
                        .opacity(o),
                    holder,
                );
            }
            None => holder(&mut ui),
        }
        ui.finish();
    }
}

struct Shots {
    gpu: Gpu,
    renderer: Renderer,
}

impl Shots {
    fn new(gpu: Gpu) -> Self {
        let renderer =
            Renderer::windowless(&gpu, wgpu::TextureFormat::Rgba8Unorm).expect("a renderer");
        Self { gpu, renderer }
    }

    fn shot(&mut self, scene: Scene, groups: bool) -> Vec<u8> {
        let _ = &self.gpu;
        let mut core = Core::new();
        core.set_subpixel_text(false);
        core.set_opacity_groups(groups);
        scene.build(&mut core);
        let (dl, atlas) = core.output();
        self.renderer.capture(dl, atlas).expect("a capture").rgba
    }
}

/// `under` and `over` mixed by `a`, pixel by pixel: the subtree faded as
/// one picture.
fn mix(under: &[u8], over: &[u8], a: f32) -> Vec<u8> {
    under
        .iter()
        .zip(over)
        .map(|(&u, &o)| (u as f32 + (o as f32 - u as f32) * a).round() as u8)
        .collect()
}

/// The largest difference of any channel, and how many pixels differ by
/// more than `by`.
fn off(a: &[u8], b: &[u8], by: u8) -> (u8, usize) {
    let worst = a
        .iter()
        .zip(b)
        .map(|(&x, &y)| x.abs_diff(y))
        .max()
        .unwrap_or(0);
    let many = a
        .chunks(4)
        .zip(b.chunks(4))
        .filter(|(p, q)| p.iter().zip(q.iter()).any(|(&x, &y)| x.abs_diff(y) > by))
        .count();
    (worst, many)
}

/// Checks `scene` at `a` against the reference, grouped within `tol` and
/// per quad well outside it.
fn against_reference(
    shots: &mut Shots,
    scene: Scene,
    without: Scene,
    with: Scene,
    a: f32,
    tol: u8,
) {
    let reference = mix(&shots.shot(without, true), &shots.shot(with, true), a);
    let grouped = shots.shot(scene, true);
    let (worst, many) = off(&grouped, &reference, tol);
    assert!(
        many == 0,
        "grouped: {many} pixels off by more than {tol}, worst {worst}"
    );
    // A white label on the dark card, each faded on its own, misses the
    // picture by a(1 - a) of the contrast between them, 223 levels.
    let expect = (a * (1.0 - a) * 223.0 * 0.8) as u8;
    let (worst, _) = off(&shots.shot(scene, false), &reference, tol);
    assert!(
        worst > expect,
        "per quad should wash out by over {expect} at {a}, but is off by only {worst}"
    );
}

#[test]
fn a_faded_card_and_its_label_fade_as_one_picture() {
    let Some(gpu) = gpu() else { return };
    let mut shots = Shots::new(gpu);
    for a in [0.5, 0.25, 0.8] {
        against_reference(
            &mut shots,
            Scene::card(a),
            Scene::card(0.0),
            Scene::card(1.0),
            a,
            2,
        );
    }
}

#[test]
fn a_group_inside_a_group_fades_inside_its_picture() {
    let Some(gpu) = gpu() else { return };
    let mut shots = Shots::new(gpu);
    let panel = |p: f32| Scene {
        panel: Some(p),
        ..Scene::card(0.6)
    };
    against_reference(&mut shots, panel(0.5), panel(0.0), panel(1.0), 0.5, 3);
}

#[test]
fn a_turned_group_under_a_clip_is_cut_where_the_frame_is() {
    let Some(gpu) = gpu() else { return };
    let mut shots = Shots::new(gpu);
    let turned = |a: f32| Scene {
        turn: 0.03,
        cut: true,
        ..Scene::card(a)
    };
    against_reference(&mut shots, turned(0.5), turned(0.0), turned(1.0), 0.5, 2);
}
