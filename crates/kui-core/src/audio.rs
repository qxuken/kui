//! Audio as data. Sounds are host-registered resources ([`SoundId`], see
//! [`crate::resources`]); playing one is a command the frame driver drains
//! ([`AudioCommand`] via `Core::take_audio_commands`) and applies to a real
//! device — the core never touches one, so headless drivers simply never
//! drain and tests assert on the queue, the same shape as window commands.
//!
//! Three ways in:
//! - declarative props: `NodeSpec::click_sound` / `hover_sound` play when
//!   the node is clicked / the pointer enters it;
//! - the `audio` element (`Core::audio_node`): a playback retained by node
//!   key — present means playing (once, or looped), gone means stopped,
//!   `volume` / `paused` changes apply live, like an HTML `<audio autoplay>`;
//!   `finish` changes what *gone* means, releasing the playback to play
//!   itself out instead of stopping it ([`AudioSpec::finish`]);
//! - imperative calls (`Core::play`, `stop`, `set_volume`, `pause`,
//!   `resume`, `set_master_volume`) for hosts that hold the core.
//!
//! A playback started with a tag comes back as
//! `{kind="sound", phase="ended", playback, tag}` on the origin that started
//! it once the driver reports it finished (`Core::audio_ended`) — not when
//! something stopped it.

use rustc_hash::FxHashMap;

use crate::input::UiEvent;
use crate::key::Key;
use crate::resources::SoundId;
use crate::tree::OriginId;
use crate::value::Value;
use crate::window::WindowId;

/// One playback instance. Allocated by the core when the play command is
/// queued, so callers get it synchronously without a driver round trip;
/// 0 is never issued.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PlaybackId(pub u64);

/// How to start a playback (`Core::play`).
#[derive(Clone, Debug, PartialEq)]
pub struct PlayOptions {
    /// Linear amplitude, 0..1 (1 = as recorded).
    pub volume: f32,
    pub looped: bool,
    /// Fade in from silence over this many ms (0 = none).
    pub fade_in_ms: f32,
    /// Carried back in the `ended` event; `None` = no event.
    pub tag: Option<Value>,
}

impl Default for PlayOptions {
    fn default() -> Self {
        Self {
            volume: 1.0,
            looped: false,
            fade_in_ms: 0.0,
            tag: None,
        }
    }
}

impl PlayOptions {
    pub fn volume(mut self, v: f32) -> Self {
        self.volume = v;
        self
    }

    pub fn looped(mut self) -> Self {
        self.looped = true;
        self
    }

    pub fn fade_in(mut self, ms: f32) -> Self {
        self.fade_in_ms = ms;
        self
    }

    /// Asks for an `ended` event carrying this tag.
    pub fn tag(mut self, tag: impl Into<Value>) -> Self {
        self.tag = Some(tag.into());
        self
    }
}

/// What an `audio` node declares each frame (`Core::audio_node`).
#[derive(Clone, Debug, PartialEq)]
pub struct AudioSpec {
    pub src: SoundId,
    /// Linear amplitude, 0..1; changes apply to the running playback.
    pub volume: f32,
    pub looped: bool,
    /// Holds the playback (resumes when cleared).
    pub paused: bool,
    /// What the node going away means: `false` stops the playback, `true`
    /// releases it — it finishes on its own. See [`AudioSpec::finish`].
    pub finish: bool,
    /// Carried back in the `ended` event; `None` = no event.
    pub tag: Option<Value>,
}

impl AudioSpec {
    pub fn new(src: SoundId) -> Self {
        Self {
            src,
            volume: 1.0,
            looped: false,
            paused: false,
            finish: false,
            tag: None,
        }
    }

    pub fn volume(mut self, v: f32) -> Self {
        self.volume = v;
        self
    }

    pub fn looped(mut self) -> Self {
        self.looped = true;
        self
    }

    pub fn paused(mut self, paused: bool) -> Self {
        self.paused = paused;
        self
    }

    /// The node's *removal* releases the playback instead of stopping it:
    /// it plays to its end, and a `tag` still reports `ended` when it gets
    /// there. Only removal changes — a `looped` playback still stops (it
    /// has no end to reach), and a changed `src` still restarts, since that
    /// is a replacement rather than a departure. A playback that is
    /// `paused` when its node goes has nothing to finish and the driver
    /// holds it forever, so a view that pauses should stop rather than
    /// release. Without this, a one-shot the view wants heard whole has to
    /// stay declared for the asset's length, which the view does not know.
    pub fn finish(mut self) -> Self {
        self.finish = true;
        self
    }

    pub fn tag(mut self, tag: impl Into<Value>) -> Self {
        self.tag = Some(tag.into());
        self
    }
}

/// An audio intent for the frame driver. Durations are ms; volumes are
/// linear amplitude. Drivers ignore playbacks they no longer hold.
#[derive(Clone, Debug, PartialEq)]
pub enum AudioCommand {
    Play {
        playback: PlaybackId,
        sound: SoundId,
        volume: f32,
        looped: bool,
        fade_in_ms: f32,
    },
    Stop {
        playback: PlaybackId,
        fade_ms: f32,
    },
    SetVolume {
        playback: PlaybackId,
        volume: f32,
        tween_ms: f32,
    },
    Pause {
        playback: PlaybackId,
        fade_ms: f32,
    },
    Resume {
        playback: PlaybackId,
        fade_ms: f32,
    },
    MasterVolume {
        volume: f32,
        tween_ms: f32,
    },
    /// The sound was unregistered: drop any decoded copy.
    Unload {
        sound: SoundId,
    },
}

/// A playback that asked for an `ended` event.
struct Tagged {
    origin: OriginId,
    key: Key,
    tag: Value,
}

/// An `audio` node's retained playback.
struct Mounted {
    playback: PlaybackId,
    spec: AudioSpec,
}

/// Playback bookkeeping on `Core`: the command queue, the tagged playbacks
/// awaiting their `ended` event, and the `audio` nodes' retained playbacks
/// (reconciled against what the frame declared in `finish_frame`).
#[derive(Default)]
pub struct AudioStore {
    next: u64,
    commands: Vec<AudioCommand>,
    tagged: FxHashMap<PlaybackId, Tagged>,
    mounted: FxHashMap<Key, Mounted>,
    /// `audio` nodes declared this frame, in tree order.
    declared: Vec<(Key, OriginId, AudioSpec)>,
}

impl AudioStore {
    fn alloc(&mut self) -> PlaybackId {
        self.next += 1;
        PlaybackId(self.next)
    }

    /// Queues a play; `origin`/`key` say where an `ended` event lands.
    pub(crate) fn play(
        &mut self,
        origin: OriginId,
        key: Key,
        sound: SoundId,
        opts: PlayOptions,
    ) -> PlaybackId {
        let playback = self.alloc();
        self.commands.push(AudioCommand::Play {
            playback,
            sound,
            volume: opts.volume,
            looped: opts.looped,
            fade_in_ms: opts.fade_in_ms,
        });
        if let Some(tag) = opts.tag {
            self.tagged.insert(playback, Tagged { origin, key, tag });
        }
        playback
    }

    /// Stops a playback; it will not report `ended`.
    pub(crate) fn stop(&mut self, playback: PlaybackId, fade_ms: f32) {
        self.tagged.remove(&playback);
        self.commands.push(AudioCommand::Stop { playback, fade_ms });
    }

    pub(crate) fn set_volume(&mut self, playback: PlaybackId, volume: f32, tween_ms: f32) {
        self.commands.push(AudioCommand::SetVolume {
            playback,
            volume,
            tween_ms,
        });
    }

    pub(crate) fn pause(&mut self, playback: PlaybackId, fade_ms: f32) {
        self.commands
            .push(AudioCommand::Pause { playback, fade_ms });
    }

    pub(crate) fn resume(&mut self, playback: PlaybackId, fade_ms: f32) {
        self.commands
            .push(AudioCommand::Resume { playback, fade_ms });
    }

    pub(crate) fn master_volume(&mut self, volume: f32, tween_ms: f32) {
        self.commands
            .push(AudioCommand::MasterVolume { volume, tween_ms });
    }

    pub(crate) fn unload(&mut self, sound: SoundId) {
        self.commands.push(AudioCommand::Unload { sound });
    }

    /// Drains the queued commands (what `Core::take_audio_commands` hands
    /// the driver).
    pub fn take_commands(&mut self) -> Vec<AudioCommand> {
        std::mem::take(&mut self.commands)
    }

    /// Commands queued and not yet drained.
    pub fn pending(&self) -> &[AudioCommand] {
        &self.commands
    }

    /// The playback an `audio` node holds, if it is mounted.
    pub fn playback_of(&self, key: Key) -> Option<PlaybackId> {
        self.mounted.get(&key).map(|m| m.playback)
    }

    /// Whether any `audio` node is mounted. The scene corpus's coverage
    /// derivation reads it: an `audio` element builds no tree node, so a
    /// mounted playback is the only trace one leaves.
    #[cfg(feature = "conformance")]
    pub(crate) fn any_mounted(&self) -> bool {
        !self.mounted.is_empty()
    }

    /// An `audio` node declared this frame; reconciled at `finish_frame`.
    pub(crate) fn declare(&mut self, key: Key, origin: OriginId, spec: AudioSpec) {
        self.declared.push((key, origin, spec));
    }

    /// The driver reported a playback finished on its own. Returns the
    /// `ended` event when the playback asked for one.
    pub(crate) fn ended(&mut self, playback: PlaybackId) -> Option<UiEvent> {
        let t = self.tagged.remove(&playback)?;
        Some(UiEvent {
            origin: t.origin,
            window: WindowId::MAIN,
            key: t.key,
            payload: Value::map([
                ("kind", Value::str("sound")),
                ("phase", Value::str("ended")),
                ("playback", Value::Int(playback.0 as i64)),
                ("tag", t.tag),
            ]),
        })
    }

    /// Diffs this frame's `audio` nodes against the retained playbacks:
    /// new keys start, missing keys stop, a changed `src`/`looped`
    /// restarts, `volume`/`paused` changes apply live. A one-shot that
    /// finished stays mounted silently until its node goes away — so a
    /// view re-rendering does not replay it. A missing key that asked to
    /// [`finish`](AudioSpec::finish) is released rather than stopped.
    pub(crate) fn reconcile(&mut self) {
        let declared = std::mem::take(&mut self.declared);
        let mut seen: Vec<Key> = Vec::with_capacity(declared.len());
        for (key, origin, spec) in declared {
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);
            let restart = match self.mounted.get(&key) {
                None => true,
                Some(m) => m.spec.src != spec.src || m.spec.looped != spec.looped,
            };
            if restart {
                if let Some(old) = self.mounted.remove(&key) {
                    self.stop(old.playback, 0.0);
                }
                let opts = PlayOptions {
                    volume: spec.volume,
                    looped: spec.looped,
                    fade_in_ms: 0.0,
                    tag: spec.tag.clone(),
                };
                let playback = self.play(origin, key, spec.src, opts);
                if spec.paused {
                    self.pause(playback, 0.0);
                }
                self.mounted.insert(key, Mounted { playback, spec });
                continue;
            }
            let m = self.mounted.get_mut(&key).expect("mounted");
            let playback = m.playback;
            if m.spec.volume != spec.volume {
                self.commands.push(AudioCommand::SetVolume {
                    playback,
                    volume: spec.volume,
                    tween_ms: 0.0,
                });
            }
            if m.spec.paused != spec.paused {
                self.commands.push(if spec.paused {
                    AudioCommand::Pause {
                        playback,
                        fade_ms: 0.0,
                    }
                } else {
                    AudioCommand::Resume {
                        playback,
                        fade_ms: 0.0,
                    }
                });
            }
            if m.spec.tag != spec.tag {
                match (&spec.tag, self.tagged.get_mut(&playback)) {
                    (Some(tag), Some(t)) => t.tag = tag.clone(),
                    (Some(tag), None) => {
                        self.tagged.insert(
                            playback,
                            Tagged {
                                origin,
                                key,
                                tag: tag.clone(),
                            },
                        );
                    }
                    (None, _) => {
                        self.tagged.remove(&playback);
                    }
                }
            }
            m.spec = spec;
        }
        // A departure stops the playback, unless the node asked to be
        // released — then it is forgotten here and finishes on the device,
        // keeping its `tagged` entry so `ended` still arrives. A looped one
        // is stopped whatever it asked: it has no end to run to.
        let gone: Vec<(Key, PlaybackId, bool)> = self
            .mounted
            .iter()
            .filter(|(k, _)| !seen.contains(k))
            .map(|(k, m)| (*k, m.playback, m.spec.finish && !m.spec.looped))
            .collect();
        for (key, playback, release) in gone {
            self.mounted.remove(&key);
            if !release {
                self.stop(playback, 0.0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::{Resources, SessionId};

    fn sound() -> SoundId {
        Resources::new(SessionId::next()).add_sound(vec![0; 4])
    }

    #[test]
    fn play_allocates_ids_and_only_tagged_playbacks_report_ended() {
        let mut a = AudioStore::default();
        let s = sound();
        let quiet = a.play(OriginId::HOST, Key::ROOT, s, PlayOptions::default());
        let loud = a.play(
            OriginId::HOST,
            Key::ROOT,
            s,
            PlayOptions::default().tag(Value::str("t")),
        );
        assert_ne!(quiet, loud);
        assert_eq!(a.take_commands().len(), 2);
        assert!(a.ended(quiet).is_none());
        let ev = a.ended(loud).expect("tagged playback reports ended");
        assert_eq!(
            ev.payload.get("kind").and_then(Value::as_str),
            Some("sound")
        );
        assert_eq!(ev.payload.get("tag").and_then(Value::as_str), Some("t"));
        assert_eq!(
            ev.payload.get("playback").and_then(Value::as_int),
            Some(loud.0 as i64)
        );
        // Reported once.
        assert!(a.ended(loud).is_none());
    }

    #[test]
    fn stop_cancels_the_ended_event() {
        let mut a = AudioStore::default();
        let s = sound();
        let p = a.play(
            OriginId::HOST,
            Key::ROOT,
            s,
            PlayOptions::default().tag(Value::Null),
        );
        a.stop(p, 0.0);
        assert!(a.ended(p).is_none());
    }

    /// F28: the node's removal releases the playback, so a one-shot the
    /// view wants heard whole no longer has to stay declared for a length
    /// the view has to guess at.
    #[test]
    fn a_removed_node_that_asked_to_finish_is_not_stopped() {
        let mut a = AudioStore::default();
        let s = sound();
        let k = Key::ROOT.str("chime");
        a.declare(k, OriginId::HOST, AudioSpec::new(s).finish());
        a.reconcile();
        assert!(matches!(
            a.take_commands().as_slice(),
            [AudioCommand::Play { .. }]
        ));

        // Gone: released, not stopped — and the store forgets it, so a
        // later re-declare of the same key starts a new playback.
        a.reconcile();
        assert_eq!(a.take_commands(), vec![]);
        assert!(a.playback_of(k).is_none());
    }

    /// Release is meaningless for a loop — there is no end to run to — so
    /// the flag changes nothing and removal still stops it.
    #[test]
    fn a_removed_loop_stops_even_when_it_asked_to_finish() {
        let mut a = AudioStore::default();
        let s = sound();
        let k = Key::ROOT.str("bed");
        a.declare(k, OriginId::HOST, AudioSpec::new(s).looped().finish());
        a.reconcile();
        let p = a.playback_of(k).unwrap();
        a.take_commands();

        a.reconcile();
        assert_eq!(
            a.take_commands(),
            vec![AudioCommand::Stop {
                playback: p,
                fade_ms: 0.0
            }]
        );
    }

    /// The `ended` event is what the release hands the view instead of the
    /// guessed duration, so it has to survive the node going away — unlike
    /// a stop, which cancels it (`stop_cancels_the_ended_event`).
    #[test]
    fn a_released_playback_still_reports_ended() {
        let mut a = AudioStore::default();
        let s = sound();
        let k = Key::ROOT.str("chime");
        let spec = {
            let mut spec = AudioSpec::new(s).finish();
            spec.tag = Some(Value::str("chime"));
            spec
        };
        a.declare(k, OriginId::HOST, spec);
        a.reconcile();
        let p = a.playback_of(k).unwrap();
        a.take_commands();

        a.reconcile();
        assert_eq!(a.take_commands(), vec![]);
        let ev = a.ended(p).expect("a released playback still reports ended");
        assert_eq!(ev.key, k);
        assert_eq!(ev.payload.get("tag").and_then(Value::as_str), Some("chime"));
    }

    #[test]
    fn audio_nodes_reconcile_by_key() {
        let mut a = AudioStore::default();
        let s = sound();
        let k = Key::ROOT.str("music");
        a.declare(k, OriginId::HOST, AudioSpec::new(s).looped());
        a.reconcile();
        let cmds = a.take_commands();
        assert!(matches!(
            cmds.as_slice(),
            [AudioCommand::Play { looped: true, .. }]
        ));
        let p = a.playback_of(k).unwrap();

        // Same declaration: nothing.
        a.declare(k, OriginId::HOST, AudioSpec::new(s).looped());
        a.reconcile();
        assert!(a.take_commands().is_empty());

        // Volume + pause apply live.
        a.declare(
            k,
            OriginId::HOST,
            AudioSpec::new(s).looped().volume(0.5).paused(true),
        );
        a.reconcile();
        let cmds = a.take_commands();
        assert_eq!(
            cmds,
            vec![
                AudioCommand::SetVolume {
                    playback: p,
                    volume: 0.5,
                    tween_ms: 0.0
                },
                AudioCommand::Pause {
                    playback: p,
                    fade_ms: 0.0
                }
            ]
        );

        // Gone: stopped.
        a.reconcile();
        assert_eq!(
            a.take_commands(),
            vec![AudioCommand::Stop {
                playback: p,
                fade_ms: 0.0
            }]
        );
        assert!(a.playback_of(k).is_none());
    }

    #[test]
    fn changing_src_restarts() {
        let mut a = AudioStore::default();
        let mut r = Resources::new(SessionId::next());
        let (s1, s2) = (r.add_sound(vec![0; 4]), r.add_sound(vec![1; 4]));
        let k = Key::ROOT.str("fx");
        a.declare(k, OriginId::HOST, AudioSpec::new(s1));
        a.reconcile();
        let p1 = a.playback_of(k).unwrap();
        a.take_commands();
        a.declare(k, OriginId::HOST, AudioSpec::new(s2));
        a.reconcile();
        let cmds = a.take_commands();
        assert!(matches!(
            cmds.as_slice(),
            [
                AudioCommand::Stop { playback, .. },
                AudioCommand::Play { sound, .. }
            ] if *playback == p1 && *sound == s2
        ));
    }
}
