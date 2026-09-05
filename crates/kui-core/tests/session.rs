//! A `Session` is what a set of windows share (see
//! `docs/adr/0004-multi-window.md` decision 2): the font database, the
//! resource registry and the audio store. A handle registered through one
//! `Core` has to mean the same thing — and draw, and play — in every other
//! `Core` built against the same session, while `Core::new()` stays a
//! session of one — and a handle that crosses from one session to another
//! is a detected miss (`foreign-resource`), never an alias of whatever
//! the other session registered first.

use kui_core::diag::FOREIGN_RESOURCE;
use kui_core::{Core, NodeSpec, QuadKind, Session, Size, Sizing, TextStyle, Warning};

/// The `foreign-resource` lines among a drain (the unlabeled test images
/// draw an `image-without-label` too, which is not what is under test).
fn codes(ws: &[Warning]) -> Vec<&'static str> {
    ws.iter()
        .map(|w| w.code)
        .filter(|c| *c == FOREIGN_RESOURCE)
        .collect()
}

fn foreign_line(ws: &[Warning]) -> &Warning {
    ws.iter()
        .find(|w| w.code == FOREIGN_RESOURCE)
        .expect("a foreign-resource line")
}

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
    // One session: the handle is at home in both, and nothing says
    // otherwise.
    assert!(codes(&a.take_warnings()).is_empty());
    assert!(codes(&b.take_warnings()).is_empty());

    // Each window packs it into its own atlas, at its own size.
    a.remove_image(id);
    assert_eq!(image_quads(&mut a, id), 0);
    assert_eq!(image_quads(&mut b, id), 0, "the registry is one registry");
    // Removed is removed, not foreign: still nothing to report.
    assert!(codes(&a.take_warnings()).is_empty());
    assert!(codes(&b.take_warnings()).is_empty());
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
    assert_ne!(a.session().id(), b.session().id());

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
    assert_eq!(codes(&b.take_warnings()), [FOREIGN_RESOURCE]);
}

/// The alias case, which is what the guard exists for: two sessions each
/// register their first image, so a per-session registry would hand out
/// the same bits for both and the wrong one would draw without a word.
#[test]
fn an_image_from_another_session_draws_nothing_and_warns_once() {
    let mut a = Core::new();
    let mut b = Core::new();
    let theirs = a.resources.add_image(40, 20, rgba(40, 20));
    let mine = b.resources.add_image(80, 10, rgba(80, 10));
    assert_ne!(theirs, mine, "handles are unique to the process");

    // b asked for a's handle: not b's own first image, not anything.
    let mut ui = b.frame(Size::new(400.0, 300.0), 1.0);
    ui.image(theirs, NodeSpec::column());
    ui.finish();
    let (dl, _) = b.output();
    assert!(
        dl.quads.iter().all(|q| q.kind != QuadKind::Image),
        "a foreign handle draws nothing, not the other session's image"
    );
    let ws = b.take_warnings();
    assert_eq!(codes(&ws), [FOREIGN_RESOURCE]);
    let msg = &foreign_line(&ws).message;
    assert!(msg.starts_with("image handle"), "{msg}");
    assert!(
        msg.contains(&format!("session #{}", a.session().id().0))
            && msg.contains(&format!("session #{}", b.session().id().0)),
        "{msg}"
    );
    assert!(msg.contains("Core::new_in"), "{msg}");

    // The same mistake next frame is the same line, already reported.
    assert_eq!(image_quads(&mut b, theirs), 0);
    assert!(codes(&b.take_warnings()).is_empty());
    // a is untouched: its handle still draws there, and it has nothing to
    // say about what b did.
    assert_eq!(image_quads(&mut a, theirs), 1);
    assert!(codes(&a.take_warnings()).is_empty());
    // b's own handle is unaffected.
    assert_eq!(image_quads(&mut b, mine), 1);
    assert!(codes(&b.take_warnings()).is_empty());
}

#[test]
fn a_font_from_another_session_shapes_as_sans_and_warns() {
    let mut a = Core::new();
    let mut b = Core::new();
    let Some(family) = a.system_font_families().into_iter().next() else {
        return; // no installed font on this machine
    };
    let Some(id) = a.add_system_font(&family) else {
        return;
    };

    // b shapes the text anyway (sans-serif), and says why.
    assert!(glyph_quads(&mut b, TextStyle::new(20.0).font(id)) > 0);
    assert_eq!(b.font_family(id), None, "b's mirror has no such font");
    let ws = b.take_warnings();
    assert_eq!(codes(&ws), [FOREIGN_RESOURCE]);
    let msg = &foreign_line(&ws).message;
    assert!(msg.starts_with("font handle"), "{msg}");
    // The owner shapes with it and says nothing.
    assert!(glyph_quads(&mut a, TextStyle::new(20.0).font(id)) > 0);
    assert!(codes(&a.take_warnings()).is_empty());
}

#[test]
fn a_sound_from_another_session_warns_when_played() {
    let mut a = Core::new();
    let mut b = Core::new();
    let sound = a.add_sound(vec![0u8; 32]);
    b.play(sound, Default::default());
    let ws = b.take_warnings();
    assert_eq!(codes(&ws), [FOREIGN_RESOURCE]);
    let msg = &foreign_line(&ws).message;
    assert!(msg.starts_with("sound handle"), "{msg}");
    // The driver would have resolved it the same way: nothing to play.
    assert!(b.resources.sound(sound).is_none());
    assert!(codes(&b.take_warnings()).is_empty(), "one line per handle");
    // Removing it through the wrong session touches nothing there.
    b.remove_sound(sound);
    assert!(a.resources.sound(sound).is_some());
}

/// The registry keeps the hits, so whichever core of a session drains
/// first reports them — a handle another window resolved included.
#[test]
fn a_foreign_hit_is_reported_by_whichever_window_drains() {
    let other = Core::new();
    let theirs = other.resources.add_image(4, 4, rgba(4, 4));
    let session = Session::new();
    let mut a = Core::new_in(&session);
    let mut b = Core::new_in(&session);
    assert_eq!(image_quads(&mut a, theirs), 0);
    assert_eq!(codes(&b.take_warnings()), [FOREIGN_RESOURCE]);
    assert!(codes(&a.take_warnings()).is_empty(), "drained once, by b");
}
