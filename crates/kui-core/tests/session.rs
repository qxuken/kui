//! A `Session` is what a set of windows share (see
//! `docs/adr/0004-multi-window.md` decision 2): the font database, the
//! resource registry and the audio store. A handle registered through one
//! `Core` has to mean the same thing — and draw, and play — in every other
//! `Core` built against the same session, while `Core::new()` stays a
//! session of one.

use kui_core::{Core, NodeSpec, QuadKind, Session, Size, Sizing, TextStyle};

fn rgba(w: u32, h: u32) -> Vec<u8> {
    vec![0x80; (w * h * 4) as usize]
}

fn image_quads(core: &mut Core, id: kui_core::ImageId) -> usize {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.image(id, NodeSpec::column());
    ui.finish();
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .filter(|q| q.kind == QuadKind::Image)
        .count()
}

fn glyph_quads(core: &mut Core, style: TextStyle) -> usize {
    let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
    ui.with(NodeSpec::column(), |ui| ui.text("Fonts", style));
    ui.finish();
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .filter(|q| q.kind != QuadKind::Solid)
        .count()
}

#[test]
fn an_image_registered_in_one_window_draws_in_the_other() {
    let session = Session::new();
    let mut a = Core::new_in(&session);
    let mut b = Core::new_in(&session);
    assert!(a.session().is(b.session()));
    assert!(session.is(a.session()));

    let id = a.resources.add_image(40, 20, rgba(40, 20));
    assert_eq!(image_quads(&mut b, id), 1, "b draws a's image");
    assert_eq!(image_quads(&mut a, id), 1);

    // Each window packs it into its own atlas, at its own size.
    a.remove_image(id);
    assert_eq!(image_quads(&mut a, id), 0);
    assert_eq!(image_quads(&mut b, id), 0, "the registry is one registry");
}

#[test]
fn a_font_registered_in_one_window_shapes_in_the_other() {
    let session = Session::new();
    let mut a = Core::new_in(&session);
    let mut b = Core::new_in(&session);

    let candidates = [
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        "/System/Library/Fonts/Helvetica.ttc",
        "C:/Windows/Fonts/arial.ttf",
    ];
    let Some(bytes) = candidates.iter().find_map(|p| std::fs::read(p).ok()) else {
        return; // no known font file on this machine
    };
    let id = a.add_font_data(bytes).expect("a real font file registers");
    let family = a.font_family(id).expect("family name recorded").to_string();

    // The name mirror is per window and refreshed by the frame, so b sees
    // it once b has drawn one.
    assert!(glyph_quads(&mut b, TextStyle::new(20.0).font(id)) > 0);
    assert_eq!(b.font_family(id), Some(family.as_str()));
    assert!(
        b.add_system_font(&family).is_some(),
        "a's faces are in the session's font database"
    );
}

#[test]
fn a_sound_registered_in_one_window_plays_from_the_other() {
    let session = Session::new();
    let mut a = Core::new_in(&session);
    let mut b = Core::new_in(&session);

    let sound = a.add_sound(vec![0u8; 32]);
    b.play(sound, Default::default());
    // One device, one queue: whichever window drains gets the command, and
    // it drains once.
    assert_eq!(a.take_audio_commands().len(), 1);
    assert!(b.take_audio_commands().is_empty());
}

#[test]
fn core_new_is_a_session_of_one() {
    let mut a = Core::new();
    let mut b = Core::new();
    assert!(!a.session().is(b.session()));

    let id = a.resources.add_image(40, 20, rgba(40, 20));
    assert_eq!(image_quads(&mut a, id), 1);
    // b's registry never heard of the handle, so its image node draws
    // nothing rather than somebody else's pixels.
    let mut ui = b.frame(Size::new(400.0, 300.0), 1.0);
    ui.image(
        id,
        NodeSpec::column()
            .width(Sizing::Fixed(40.0))
            .height(Sizing::Fixed(20.0)),
    );
    ui.finish();
    let (dl, _) = b.output();
    assert!(dl.quads.iter().all(|q| q.kind != QuadKind::Image));
}
