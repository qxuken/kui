//! `kui_native::capture` (backlog C55): a core's finished frame as RGBA,
//! drawn again on the process's windowless device. A headless core here,
//! the way a test captures; skipped, saying so, where there is no adapter,
//! and failed there where `KUI_REQUIRE_GPU` is set, as CI sets it over
//! Mesa's software Vulkan.

use kui_native::{Color, Core, NodeSpec, Size, capture};

/// `None` when this machine has no GPU adapter at all, which is the one
/// failure a test cannot do anything about.
fn shot(core: &Core) -> Option<kui_native::Capture> {
    match capture(core) {
        Ok(c) => Some(c),
        Err(e) if e.starts_with("no GPU") => {
            assert!(
                std::env::var_os("KUI_REQUIRE_GPU").is_none_or(|v| v.is_empty()),
                "KUI_REQUIRE_GPU is set, but {e}"
            );
            eprintln!("skipped: {e}");
            None
        }
        Err(e) => panic!("{e}"),
    }
}

fn at(c: &kui_native::Capture, x: u32, y: u32) -> [u8; 4] {
    let o = ((y * c.width + x) * 4) as usize;
    c.rgba[o..o + 4].try_into().unwrap()
}

#[test]
fn a_headless_core_captures_its_frame_over_the_theme_s_ground() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(30.0, 20.0), 2.0);
    ui.configure_root(NodeSpec::column().pad(5.0));
    ui.leaf_keyed(
        "box",
        NodeSpec::column()
            .size(10.0, 5.0)
            .bg(Color::hex(0x00ff00ff)),
    );
    ui.finish();
    let Some(c) = shot(&core) else { return };
    assert_eq!((c.width, c.height, c.scale), (60, 40, 2.0));
    assert_eq!(at(&c, 20, 15), [0, 255, 0, 255], "the box, in physical px");
    // The root paints nothing: what shows is the clear, the theme's bg,
    // as an opaque window's is.
    let bg = core.theme().bg;
    let byte = |v: f32| (v * 255.0).round() as u8;
    let want = [byte(bg.r), byte(bg.g), byte(bg.b), 255];
    let got = at(&c, 55, 35);
    for i in 0..4 {
        assert!(got[i].abs_diff(want[i]) <= 1, "{got:?} against {want:?}");
    }
    // A second capture reuses the device and says the same.
    assert_eq!(shot(&core).expect("again").rgba, c.rgba);
}

#[test]
fn a_core_with_no_finished_frame_has_nothing_to_capture() {
    let core = Core::new();
    let e = capture(&core).expect_err("no frame yet");
    assert!(e.contains("no finished frame"), "{e}");
}
