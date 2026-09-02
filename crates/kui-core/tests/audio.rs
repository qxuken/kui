//! Audio as data, end to end through a live `Core`: declarative click /
//! hover sounds become play commands, `audio` nodes hold a playback for as
//! long as the view declares them, and a tagged playback the driver reports
//! finished comes back as a `sound` event.

use kui_core::{
    AudioCommand, AudioSpec, Core, InputEvent, Key, NodeSpec, PlayOptions, Size, Sizing, SoundId,
    Value, Vec2,
};

/// One frame: a button-sized node with `spec`, and optionally an `audio`
/// node next to it. Returns the button's key and the audio node's key.
fn frame(core: &mut Core, spec: NodeSpec, audio: Option<AudioSpec>) -> (Key, Key) {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    let mut keys = (Key(0), Key(0));
    ui.with(NodeSpec::column(), |ui| {
        keys.0 = ui.child_key("btn");
        ui.with_keyed(
            "btn",
            spec.width(Sizing::Fixed(40.0)).height(Sizing::Fixed(20.0)),
            |_| {},
        );
        if let Some(a) = audio {
            keys.1 = ui.audio_keyed("music", a);
        }
    });
    ui.finish();
    keys
}

fn sound(core: &mut Core) -> SoundId {
    core.add_sound(b"RIFF....WAVE".to_vec())
}

#[test]
fn click_sound_plays_on_click_alongside_the_click_event() {
    let mut core = Core::new();
    let s = sound(&mut core);
    frame(
        &mut core,
        NodeSpec::column()
            .click_sound(s)
            .on_click(Value::str("hit")),
        None,
    );
    core.handle_input(InputEvent::CursorMoved(Vec2::new(10.0, 10.0)));
    core.handle_input(InputEvent::MouseDown(1));
    let evs = core.handle_input(InputEvent::MouseUp);
    assert_eq!(evs.len(), 1, "the click event still fires");
    let cmds = core.take_audio_commands();
    assert!(
        matches!(cmds.as_slice(), [AudioCommand::Play { sound, looped: false, .. }] if *sound == s),
        "{cmds:?}"
    );
    assert!(core.take_audio_commands().is_empty(), "drained");
}

#[test]
fn click_sound_alone_makes_the_node_hit_tracked_but_emits_no_event() {
    let mut core = Core::new();
    let s = sound(&mut core);
    let (key, _) = frame(&mut core, NodeSpec::column().click_sound(s), None);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(10.0, 10.0)));
    assert!(core.is_hovered(key));
    core.handle_input(InputEvent::MouseDown(1));
    let evs = core.handle_input(InputEvent::MouseUp);
    assert!(evs.is_empty());
    assert_eq!(core.take_audio_commands().len(), 1);
}

#[test]
fn hover_sound_plays_on_enter_only() {
    let mut core = Core::new();
    let s = sound(&mut core);
    frame(&mut core, NodeSpec::column().hover_sound(s), None);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(10.0, 10.0)));
    assert_eq!(core.take_audio_commands().len(), 1, "enter plays");
    core.handle_input(InputEvent::CursorMoved(Vec2::new(12.0, 12.0)));
    assert!(
        core.take_audio_commands().is_empty(),
        "moving inside is silent"
    );
    core.handle_input(InputEvent::CursorMoved(Vec2::new(200.0, 200.0)));
    assert!(core.take_audio_commands().is_empty(), "leaving is silent");
    core.handle_input(InputEvent::CursorMoved(Vec2::new(10.0, 10.0)));
    assert_eq!(
        core.take_audio_commands().len(),
        1,
        "re-entering plays again"
    );
}

#[test]
fn hover_sound_plays_when_a_frame_moves_the_node_under_a_still_cursor() {
    let mut core = Core::new();
    let s = sound(&mut core);
    // Nothing under the cursor yet.
    frame(&mut core, NodeSpec::column(), None);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(10.0, 10.0)));
    assert!(core.take_audio_commands().is_empty());
    // The next frame puts a hover-sound node there.
    frame(&mut core, NodeSpec::column().hover_sound(s), None);
    assert_eq!(core.take_audio_commands().len(), 1);
}

#[test]
fn audio_node_plays_while_declared_and_stops_when_gone() {
    let mut core = Core::new();
    let s = sound(&mut core);
    frame(
        &mut core,
        NodeSpec::column(),
        Some(AudioSpec::new(s).looped()),
    );
    let cmds = core.take_audio_commands();
    let playback = match cmds.as_slice() {
        [
            AudioCommand::Play {
                playback,
                sound,
                looped: true,
                ..
            },
        ] if *sound == s => *playback,
        other => panic!("{other:?}"),
    };
    // Re-rendering the same view is silent.
    frame(
        &mut core,
        NodeSpec::column(),
        Some(AudioSpec::new(s).looped()),
    );
    assert!(core.take_audio_commands().is_empty());
    // Volume and pause changes apply to the running playback.
    frame(
        &mut core,
        NodeSpec::column(),
        Some(AudioSpec::new(s).looped().volume(0.25).paused(true)),
    );
    assert_eq!(
        core.take_audio_commands(),
        vec![
            AudioCommand::SetVolume {
                playback,
                volume: 0.25,
                tween_ms: 0.0
            },
            AudioCommand::Pause {
                playback,
                fade_ms: 0.0
            }
        ]
    );
    // Dropping the node stops it.
    frame(&mut core, NodeSpec::column(), None);
    assert_eq!(
        core.take_audio_commands(),
        vec![AudioCommand::Stop {
            playback,
            fade_ms: 0.0
        }]
    );
}

#[test]
fn tagged_playback_reports_ended_as_a_pending_event() {
    let mut core = Core::new();
    let s = sound(&mut core);
    let playback = core.play(s, PlayOptions::default().tag(Value::str("chime")));
    assert_eq!(core.take_audio_commands().len(), 1);
    core.audio_ended(playback);
    let evs = core.take_pending_events();
    assert_eq!(evs.len(), 1);
    let p = &evs[0].payload;
    assert_eq!(p.get("kind").and_then(Value::as_str), Some("sound"));
    assert_eq!(p.get("phase").and_then(Value::as_str), Some("ended"));
    assert_eq!(p.get("tag").and_then(Value::as_str), Some("chime"));
    assert_eq!(
        p.get("playback").and_then(Value::as_int),
        Some(playback.0 as i64)
    );
    // Untagged playbacks stay silent, stopped ones too.
    let quiet = core.play(s, PlayOptions::default());
    core.audio_ended(quiet);
    let loud = core.play(s, PlayOptions::default().tag(Value::Null));
    core.stop(loud, 100.0);
    core.audio_ended(loud);
    assert!(core.take_pending_events().is_empty());
}

#[test]
fn audio_node_tag_rides_the_ended_event_on_the_node_key() {
    let mut core = Core::new();
    let s = sound(&mut core);
    let (_, music) = frame(
        &mut core,
        NodeSpec::column(),
        Some(AudioSpec::new(s).tag(Value::str("done"))),
    );
    let playback = match core.take_audio_commands().as_slice() {
        [AudioCommand::Play { playback, .. }] => *playback,
        other => panic!("{other:?}"),
    };
    core.audio_ended(playback);
    let evs = core.take_pending_events();
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].key, music, "the event lands on the audio node's key");
    assert_eq!(
        evs[0].payload.get("tag").and_then(Value::as_str),
        Some("done")
    );
    // A finished one-shot stays mounted silently: re-rendering does not
    // replay it, and its node going away emits a (harmless) stop.
    frame(
        &mut core,
        NodeSpec::column(),
        Some(AudioSpec::new(s).tag(Value::str("done"))),
    );
    assert!(core.take_audio_commands().is_empty());
}

#[test]
fn removing_a_sound_unloads_it() {
    let mut core = Core::new();
    let s = sound(&mut core);
    core.remove_sound(s);
    assert_eq!(
        core.take_audio_commands(),
        vec![AudioCommand::Unload { sound: s }]
    );
    core.remove_sound(s);
    assert!(
        core.take_audio_commands().is_empty(),
        "stale handle: nothing"
    );
    assert!(core.resources.sound(s).is_none());
}
