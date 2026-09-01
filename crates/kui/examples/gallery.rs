//! Images in the display list: host-registered RGBA pixels drawn through
//! the same atlas and draw call as everything else. Shows intrinsic (Fit)
//! sizing, aspect-preserving responsive width, rounded corners, and alpha.
//!
//! Run: cargo run -p kui --example gallery

use kui::{App, Color, ImageId, NodeSpec, Sizing, TextStyle, Ui};

/// A procedural "photo": vertical sky gradient with a sun disc.
fn sky(w: u32, h: u32) -> Vec<u8> {
    let mut px = Vec::with_capacity((w * h * 4) as usize);
    let (cx, cy, r) = (w as f32 * 0.72, h as f32 * 0.3, h as f32 * 0.16);
    for y in 0..h {
        let t = y as f32 / h as f32;
        for x in 0..w {
            let d = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
            let sun = (1.0 - ((d - r) / 6.0).clamp(0.0, 1.0)).powi(2);
            let base = [
                (30.0 + 160.0 * t) as u8,
                (60.0 + 120.0 * t) as u8,
                (120.0 + 80.0 * t) as u8,
            ];
            px.push(base[0].saturating_add((sun * 220.0) as u8));
            px.push(base[1].saturating_add((sun * 180.0) as u8));
            px.push(base[2].saturating_add((sun * 60.0) as u8));
            px.push(0xff);
        }
    }
    px
}

/// Checkerboard with transparent squares — alpha compositing over the bg.
fn checker(w: u32, h: u32) -> Vec<u8> {
    let mut px = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let on = ((x / 12) + (y / 12)) % 2 == 0;
            px.extend_from_slice(if on { &[0xd8, 0x86, 0x3b, 0xff] } else { &[0, 0, 0, 0] });
        }
    }
    px
}

struct Gallery {
    sky: Option<ImageId>,
    checker: Option<ImageId>,
}

impl App for Gallery {
    fn view(&mut self, ui: &mut Ui<'_>) {
        // Register once, lazily, through the escape hatch — resources are
        // long-lived core state, not per-frame data.
        let sky_id =
            *self.sky.get_or_insert_with(|| ui.core().resources.add_image(480, 270, sky(480, 270)));
        let checker_id = *self
            .checker
            .get_or_insert_with(|| ui.core().resources.add_image(96, 96, checker(96, 96)));

        ui.configure_root(NodeSpec::column().fill().pad(24.0).gap(16.0).scroll_y());
        kui::widgets::titlebar(ui, "kui — gallery");
        let muted = TextStyle::new(12.0).color(Color::rgb8(0x8a, 0x8f, 0xa3));

        ui.text("Grow width + Fit height: rescales with the window, keeps aspect", muted);
        ui.image(
            sky_id,
            NodeSpec::column().width(Sizing::Grow(1.0)).max_width(720.0).radius(12.0),
        );

        ui.text("Intrinsic size, rounded, over a colored card (alpha shows through)", muted);
        ui.with(
            NodeSpec::row().pad(16.0).gap(16.0).bg(Color::rgb8(0x14, 0x16, 0x1e)).radius(10.0),
            |ui| {
                ui.image(checker_id, NodeSpec::column().radius(8.0));
                ui.image(
                    checker_id,
                    NodeSpec::column()
                        .width(Sizing::Fixed(48.0))
                        .height(Sizing::Fixed(96.0))
                        .radius(8.0),
                );
                ui.text("same image, intrinsic and stretched", muted);
            },
        );

        kui::widgets::latency_hud(ui);
    }
}

fn main() {
    kui::app("kui — gallery").run(Gallery { sky: None, checker: None }).unwrap();
}
