//! A `Session` is what a set of windows share (see
//! `docs/adr/0004-multi-window.md` decision 2): the font database, the
//! resource registry and the audio store. A handle registered through one
//! `Core` has to mean the same thing — and draw, and play — in every other
//! `Core` built against the same session, while `Core::new()` stays a
//! session of one — and a handle that crosses from one session to another
//! is a detected miss (`foreign-resource`), never an alias of whatever
//! the other session registered first.

use kui_core::diag::FOREIGN_RESOURCE;
use kui_core::{
    AudioCommand, AudioSpec, Core, Key, NodeSpec, QuadKind, Session, Size, Sizing, TextStyle,
    Value, Warning, WindowId,
};

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

/// AR8: a texture lives on the device every window shares, so a removal
/// has to reach *a* display list — not this core's next one, which a
/// window that closes never builds. The list is the session's and the
/// next frame any window builds carries it; the removing window's own
/// next frame carries nothing twice.
#[test]
fn a_texture_removed_through_one_window_is_dropped_by_whichever_draws_next() {
    let session = Session::new();
    let mut a = Core::new_in(&session);
    let mut b = Core::new_in(&session);

    let id = a.resources.add_image(4, 4, rgba(4, 4));
    assert!(a.update_image(id, 8, 2, rgba(8, 2)), "texture-backed now");
    assert_eq!(image_quads(&mut a, id), 0, "a texture is not an atlas quad");
    assert_eq!(a.output().0.textures.len(), 1);

    // Removed through a, and a never draws again.
    a.remove_image(id);
    assert_eq!(image_quads(&mut b, id), 0);
    assert_eq!(
        b.output().0.dropped_textures,
        vec![id],
        "b's list carries the drop a's window would have"
    );
    // Once: the next list — either window's — is clean.
    image_quads(&mut b, id);
    assert!(b.output().0.dropped_textures.is_empty());
    image_quads(&mut a, id);
    assert!(a.output().0.dropped_textures.is_empty());
}

/// The same for a fragment, whose pipelines the backend built per handle
/// and, before this, never freed.
#[test]
fn a_fragment_removed_through_one_window_is_dropped_by_whichever_draws_next() {
    let session = Session::new();
    let mut a = Core::new_in(&session);
    let mut b = Core::new_in(&session);

    let f = a
        .add_fragment(
            "fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> { return vec4<f32>(1.0); }",
        )
        .expect("a valid fragment");
    a.remove_fragment(f);
    image_quads(&mut b, a.resources.add_image(1, 1, rgba(1, 1)));
    assert_eq!(b.output().0.dropped_fragments, vec![f]);
    image_quads(&mut b, a.resources.add_image(1, 1, rgba(1, 1)));
    assert!(b.output().0.dropped_fragments.is_empty());
}

/// An atlas-backed image is packed into every window's own atlas, and
/// the removing core evicts its own slot at once; the others learn at
/// their next frame, from the revision, rather than keeping a slot for
/// a handle that can never be drawn again.
#[test]
fn an_image_removed_through_one_window_leaves_the_other_window_s_atlas() {
    let session = Session::new();
    let mut a = Core::new_in(&session);
    let mut b = Core::new_in(&session);

    let id = a.resources.add_image(40, 20, rgba(40, 20));
    assert_eq!(image_quads(&mut b, id), 1);
    assert!(b.atlas.has_image(id), "packed into b's atlas");

    a.remove_image(id);
    assert!(!a.atlas.has_image(id), "a evicted its own at once");
    assert!(b.atlas.has_image(id), "b has not framed since");
    let ui = b.frame(Size::new(400.0, 300.0), 1.0);
    ui.finish();
    assert!(!b.atlas.has_image(id), "b's next frame forgets the slot");
}

#[test]
fn a_font_registered_in_one_window_shapes_in_the_other() {
    let session = Session::new();
    let mut a = Core::new_in(&session);
    let mut b = Core::new_in(&session);

    let id = a
        .add_font_data(kui_core::testing::liga_font())
        .expect("the fixture face registers");
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

/// One frame of `core` that declares an `audio` node named `music` when
/// `audio` is given, and nothing otherwise. Returns the node's key.
fn audio_frame(core: &mut Core, audio: Option<AudioSpec>) -> Key {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    let mut key = Key::ROOT;
    ui.with(NodeSpec::column(), |ui| {
        if let Some(a) = audio {
            key = ui.audio_keyed("music", a);
        }
    });
    ui.finish();
    key
}

/// AR7: the store is the session's because the device is, but a mount is
/// one window's — it is reconciled against *that* window's frames. Before
/// this, `finish_frame` on any core diffed every mount against its own
/// frame, so a popup or a second window drawing a frame with no `<audio>`
/// in it stopped the main window's loop, and main's next frame started it
/// again from zero.
#[test]
fn a_second_window_frame_leaves_the_first_window_s_audio_node_playing() {
    let session = Session::new();
    let mut a = Core::new_in(&session);
    let mut b = Core::new_in(&session);
    b.env.window.id = WindowId(2);

    let sound = a.add_sound(vec![0u8; 32]);
    let music = audio_frame(&mut a, Some(AudioSpec::new(sound).looped()));
    let playback = match a.take_audio_commands().as_slice() {
        [AudioCommand::Play { playback, .. }] => *playback,
        other => panic!("{other:?}"),
    };
    assert_eq!(a.playback_of(music), Some(playback));

    // b's frame declares no audio: a's mount is not b's to reconcile.
    audio_frame(&mut b, None);
    assert!(
        b.take_audio_commands().is_empty(),
        "b's empty frame must not stop a's loop"
    );
    assert_eq!(a.playback_of(music), Some(playback), "still mounted");
    assert_eq!(b.playback_of(music), None, "and not b's");

    // a re-declaring it is silent — the mount survived, so nothing restarts.
    audio_frame(&mut a, Some(AudioSpec::new(sound).looped()));
    assert!(a.take_audio_commands().is_empty());

    // The same key in b's frame is b's own mount: a second playback, and
    // b dropping it stops b's, not a's.
    audio_frame(&mut b, Some(AudioSpec::new(sound).looped()));
    let b_playback = match a.take_audio_commands().as_slice() {
        [AudioCommand::Play { playback, .. }] => *playback,
        other => panic!("{other:?}"),
    };
    assert_ne!(b_playback, playback);
    assert_eq!(b.playback_of(music), Some(b_playback));
    audio_frame(&mut b, None);
    assert_eq!(
        a.take_audio_commands(),
        vec![AudioCommand::Stop {
            playback: b_playback,
            fade_ms: 0.0
        }]
    );
    assert_eq!(a.playback_of(music), Some(playback));

    // And a dropping its own stops it.
    audio_frame(&mut a, None);
    assert_eq!(
        a.take_audio_commands(),
        vec![AudioCommand::Stop {
            playback,
            fade_ms: 0.0
        }]
    );
}

/// A window's mounts are its own to reconcile (AR7) — so a window that
/// closes, and finishes no more frames, has to let them go on its way out,
/// or a popup's looped bed plays on after the popup is gone (found in the
/// code review of the round). Both ways a window closes: the OS's close
/// (`window_closed`) and the app no longer declaring it.
#[test]
fn a_closed_window_s_audio_nodes_stop_with_it() {
    let session = Session::new();
    let mut a = Core::new_in(&session);
    let sound = a.add_sound(vec![0u8; 32]);
    let declare = |core: &mut Core, popup: bool| {
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        if popup {
            ui.window("popup", kui_core::WindowConfig::sized(200.0, 100.0));
        }
        ui.finish();
    };
    declare(&mut a, true);
    let id = match a.take_window_commands().as_slice() {
        [kui_core::WindowCommand::Open { id, .. }] => *id,
        other => panic!("{other:?}"),
    };
    let mut b = Core::new_in(&session);
    b.env.window.id = id;
    audio_frame(&mut b, Some(AudioSpec::new(sound).looped()));
    let playback = match a.take_audio_commands().as_slice() {
        [AudioCommand::Play { playback, .. }] => *playback,
        other => panic!("{other:?}"),
    };

    // The OS closes it: the loop stops with it.
    a.window_closed(id);
    assert_eq!(
        a.take_audio_commands(),
        vec![AudioCommand::Stop {
            playback,
            fade_ms: 0.0
        }]
    );

    // Declared again — a new window, its own mount — and this time the
    // app stops declaring it.
    declare(&mut a, false);
    a.take_window_commands();
    declare(&mut a, true);
    let id = match a.take_window_commands().as_slice() {
        [kui_core::WindowCommand::Open { id, .. }] => *id,
        other => panic!("{other:?}"),
    };
    let mut b = Core::new_in(&session);
    b.env.window.id = id;
    audio_frame(&mut b, Some(AudioSpec::new(sound).looped()));
    let playback = match a.take_audio_commands().as_slice() {
        [AudioCommand::Play { playback, .. }] => *playback,
        other => panic!("{other:?}"),
    };
    declare(&mut a, false);
    assert!(matches!(
        a.take_window_commands().as_slice(),
        [kui_core::WindowCommand::Close(_)]
    ));
    assert_eq!(
        a.take_audio_commands(),
        vec![AudioCommand::Stop {
            playback,
            fade_ms: 0.0
        }]
    );
}

/// The `ended` and `refused` events used to be stamped `MAIN` by hand,
/// because the store did not know its window; now the mount does, and the
/// event lands on the window that declared the node whichever core the
/// driver folded it back through (the runner uses the main one).
#[test]
fn a_tagged_playback_s_ended_event_names_the_window_that_declared_it() {
    let session = Session::new();
    let mut a = Core::new_in(&session);
    let mut b = Core::new_in(&session);
    b.env.window.id = WindowId(2);

    let sound = a.add_sound(vec![0u8; 32]);
    let music = audio_frame(&mut b, Some(AudioSpec::new(sound).tag(Value::str("done"))));
    let playback = match a.take_audio_commands().as_slice() {
        [AudioCommand::Play { playback, .. }] => *playback,
        other => panic!("{other:?}"),
    };

    // Folded back through a (the main window's core, as the runner does).
    a.audio_ended(playback);
    let evs = a.take_pending_events();
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].window, WindowId(2), "b's window, not the drainer's");
    assert_eq!(evs[0].key, music);

    // A refusal the same way. The finished one-shot stays mounted until
    // its node goes, so drop it and declare it again for a fresh playback.
    audio_frame(&mut b, None);
    a.take_audio_commands();
    audio_frame(&mut b, Some(AudioSpec::new(sound).tag(Value::str("again"))));
    let playback = match a.take_audio_commands().as_slice() {
        [AudioCommand::Play { playback, .. }] => *playback,
        other => panic!("{other:?}"),
    };
    a.audio_refused(playback);
    let evs = a.take_pending_events();
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].window, WindowId(2));
    assert_eq!(
        evs[0].payload.get("phase").and_then(Value::as_str),
        Some("refused")
    );
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
    let id = a
        .add_font_data(kui_core::testing::liga_font())
        .expect("the fixture face registers");

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
