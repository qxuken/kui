//! What a level costs (`docs/adr/0044-an-image-drawn-smaller-is-drawn-from-a-level.md`):
//! the halving a photo's first small draw pays, once per session, and
//! the frame after it, which pays a lookup. A 4032×3024 photo — a
//! phone's — of noise, so nothing about the content is cheap.
//!
//! Run: cargo bench -p kui-core --bench levels

use kui_core::{Core, NodeSpec, Size};

const W: u32 = 4032;
const H: u32 = 3024;

fn photo() -> Vec<u8> {
    let mut x = 0x9e37_79b9u32;
    (0..W * H * 4)
        .map(|i| {
            if i % 4 == 3 {
                return 0xff;
            }
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            x as u8
        })
        .collect()
}

fn main() {
    divan::main();
}

/// Level 0 to level 1: three million output texels, each four in.
#[divan::bench(sample_count = 10)]
fn halve_4032x3024(b: divan::Bencher) {
    let px = photo();
    b.bench(|| kui_core::mip::halve(W, H, divan::black_box(&px)));
}

/// The first frame that draws the photo on a 400×300 card at 2×: the
/// chain to level 3 made (1, 2 and 3) and level 3 blitted into the page.
#[divan::bench(sample_count = 10)]
fn first_small_draw(b: divan::Bencher) {
    let px = photo();
    b.with_inputs(|| {
        let core = Core::new();
        let id = core.resources.add_image(W, H, px.clone());
        (core, id)
    })
    .bench_local_values(|(mut core, id)| {
        let mut ui = core.frame(Size::new(800.0, 600.0), 2.0);
        ui.image(id, NodeSpec::column().size(400.0, 300.0));
        ui.finish();
        core
    });
}

/// Every frame after: the level is in the page and on the entry.
#[divan::bench]
fn warm_small_draw(b: divan::Bencher) {
    let mut core = Core::new();
    let id = core.resources.add_image(W, H, photo());
    let frame = |core: &mut Core| {
        let mut ui = core.frame(Size::new(800.0, 600.0), 2.0);
        ui.image(id, NodeSpec::column().size(400.0, 300.0));
        ui.finish();
    };
    frame(&mut core);
    b.bench_local(|| frame(&mut core));
}
