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
//! something stopped it. The other direction is
//! [`crate::diag::TRUNCATED_PLAYBACK`]: the driver reports a stop that
//! landed on a sound still playing (`Core::audio_truncated`) and the core
//! names the node it cut off. A play the device refused — its voices all
//! held, or the sound undecodable — comes back as `phase="refused"`
//! (`Core::audio_refused`), because that playback never starts and so
//! never ends: without it a view waiting on `ended` waits forever.

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
    ///
    /// This is also what
    /// [`truncated-playback`](crate::diag::TRUNCATED_PLAYBACK) asks for: a
    /// one-shot removed mid-sound raises that warning, and the answer to
    /// it is either this flag or a node kept declared until the `ended`
    /// event. A node that means to cut the sound off says so by stopping
    /// what it started (`Core::stop`), which is not reported.
    ///
    /// A released playback is not free: it holds one of the device's 128
    /// voices until its file ends, released or not, and the 129th play is
    /// refused — reported as [`crate::diag::PLAYBACK_REFUSED`] and, for a
    /// tagged node, `{kind:"sound", phase:"refused"}` so nothing waits on
    /// an `ended` that cannot come. 128 is kira's number, not a kui budget
    /// on top of it (`MainTrackBuilder::sound_capacity` is where a setting
    /// would go); a view that releases a one-shot per keystroke of a 1.4 s
    /// file would need ninety keystrokes a second to reach it, and a loop
    /// reaches it at once — which is why a loop is stopped rather than
    /// released.
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

impl AudioCommand {
    /// The command's wire name: `play`, `stop`, `setVolume`, `pause`,
    /// `resume`, `masterVolume`, `unload`.
    pub fn kind_name(&self) -> &'static str {
        match self {
            AudioCommand::Play { .. } => "play",
            AudioCommand::Stop { .. } => "stop",
            AudioCommand::SetVolume { .. } => "setVolume",
            AudioCommand::Pause { .. } => "pause",
            AudioCommand::Resume { .. } => "resume",
            AudioCommand::MasterVolume { .. } => "masterVolume",
            AudioCommand::Unload { .. } => "unload",
        }
    }

    /// `{kind, ...}` with what the variant carries: `playback` (a small
    /// counter, an integer), `sound` (a resource id, spelled by `h`),
    /// `volume`, `loop`, and the durations in ms as `fade_in`, `fade`,
    /// `tween`.
    pub fn to_value(&self, h: crate::value::Handles) -> Value {
        let pb = |p: PlaybackId| Value::Int(p.0 as i64);
        let mut out = vec![("kind".to_string(), Value::str(self.kind_name()))];
        let mut push = |k: &str, v: Value| out.push((k.to_string(), v));
        match *self {
            AudioCommand::Play {
                playback,
                sound,
                volume,
                looped,
                fade_in_ms,
            } => {
                push("playback", pb(playback));
                push("sound", (h.id)(sound.to_ffi()));
                push("volume", Value::float(volume));
                push("loop", Value::Bool(looped));
                push("fade_in", Value::float(fade_in_ms));
            }
            AudioCommand::Stop { playback, fade_ms }
            | AudioCommand::Pause { playback, fade_ms }
            | AudioCommand::Resume { playback, fade_ms } => {
                push("playback", pb(playback));
                push("fade", Value::float(fade_ms));
            }
            AudioCommand::SetVolume {
                playback,
                volume,
                tween_ms,
            } => {
                push("playback", pb(playback));
                push("volume", Value::float(volume));
                push("tween", Value::float(tween_ms));
            }
            AudioCommand::MasterVolume { volume, tween_ms } => {
                push("volume", Value::float(volume));
                push("tween", Value::float(tween_ms));
            }
            AudioCommand::Unload { sound } => push("sound", (h.id)(sound.to_ffi())),
        }
        Value::Map(out)
    }
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

/// Why a one-shot playback was cut off — what
/// [`diag::TRUNCATED_PLAYBACK`](crate::diag::TRUNCATED_PLAYBACK) reports
/// once the driver confirms the sound was still running.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// The node declaring it went away without [`AudioSpec::finish`].
    Removed,
    /// The node changed its `src`, which replaces the playback.
    Restarted,
}

impl Why {
    /// The verb for the message ("removed at 0.5 s").
    pub(crate) fn verb(self) -> &'static str {
        match self {
            Why::Removed => "removed",
            Why::Restarted => "restarted",
        }
    }
}

/// Stops the driver has not answered for yet are remembered so a
/// truncation can name its node; a headless core has no driver to answer,
/// so the map is capped and the oldest entry — the lowest [`PlaybackId`],
/// which they are issued in — makes room for a newer one.
const MAX_STOPPED: usize = 256;

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
    /// One-shots `reconcile` stopped, awaiting the driver's word on
    /// whether they were still playing (`Core::audio_truncated`). The
    /// core cannot know that itself: `ended` is the driver's too, an
    /// untagged one-shot leaves no `tagged` entry to have heard it, and a
    /// headless `Ctx` has no driver at all — so nothing here is a warning
    /// until something answers for it.
    stopped: FxHashMap<PlaybackId, (Key, Why)>,
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

    /// Remembers a stop that may have cut a sound off, so the driver's
    /// answer has a node to land on. Bounded: at the cap the oldest
    /// unanswered stop is dropped.
    fn record_stop(&mut self, playback: PlaybackId, key: Key, why: Why) {
        if self.stopped.len() >= MAX_STOPPED
            && let Some(oldest) = self.stopped.keys().min().copied()
        {
            self.stopped.remove(&oldest);
        }
        self.stopped.insert(playback, (key, why));
    }

    /// The driver reports it stopped `playback` while the sound was still
    /// running. Returns the node it was declared on and why it was cut,
    /// once — a stop that landed after the sound ended, or one of a
    /// playback nothing recorded (an imperative `stop`, a loop, a
    /// `finish` release), answers nothing.
    pub(crate) fn truncated(&mut self, playback: PlaybackId) -> Option<(Key, Why)> {
        self.stopped.remove(&playback)
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
        // It reached its end, so a stop queued for it in the same breath
        // cut nothing off.
        self.stopped.remove(&playback);
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

    /// The driver refused a play — the device's voices are all held, or
    /// the sound did not decode. The playback never started, so it will
    /// never reach [`Self::ended`]: a tagged one is handed the same event
    /// with `phase: "refused"` instead, which unsticks a view waiting on
    /// the sound and still tells it the sound was not heard. The warning
    /// comes back whether or not anything was waiting — an untagged
    /// refusal is silent otherwise.
    ///
    /// The `audio` node's mount is left alone: unmounting it would have
    /// the next frame re-declare, replay and be refused again, one line
    /// per frame, where leaving it mounted costs one.
    pub(crate) fn refused(
        &mut self,
        playback: PlaybackId,
    ) -> (Option<UiEvent>, crate::diag::Warning) {
        let warning = crate::diag::playback_refused(self.key_of(playback), playback);
        let event = self.tagged.remove(&playback).map(|t| UiEvent {
            origin: t.origin,
            window: WindowId::MAIN,
            key: t.key,
            payload: Value::map([
                ("kind", Value::str("sound")),
                ("phase", Value::str("refused")),
                ("playback", Value::Int(playback.0 as i64)),
                ("tag", t.tag),
            ]),
        });
        (event, warning)
    }

    /// The node a warning about a playback hangs on: the node that asked
    /// for the sound when one did — a tagged playback's, or the `audio`
    /// element that mounted it — and the root otherwise, which is where an
    /// imperative `play` and a `click_sound` start from anyway.
    fn key_of(&self, playback: PlaybackId) -> Key {
        if let Some(t) = self.tagged.get(&playback) {
            return t.key;
        }
        self.mounted
            .iter()
            .find(|(_, m)| m.playback == playback)
            .map(|(k, _)| *k)
            .unwrap_or(Key::ROOT)
    }

    /// Diffs this frame's `audio` nodes against the retained playbacks:
    /// new keys start, missing keys stop, a changed `src`/`looped`
    /// restarts, `volume`/`paused` changes apply live. A one-shot that
    /// finished stays mounted silently until its node goes away — so a
    /// view re-rendering does not replay it. A missing key that asked to
    /// [`finish`](AudioSpec::finish) is released rather than stopped.
    /// Every other stop of a one-shot is remembered (see `truncated`) in
    /// case the driver says the sound was still running.
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
                    // A replaced one-shot is cut off exactly as a removed
                    // one is; `finish` does not release it (it is not a
                    // departure), so it is also the opt-out here.
                    if !old.spec.looped && !old.spec.finish {
                        self.record_stop(old.playback, key, Why::Restarted);
                    }
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
        let gone: Vec<(Key, PlaybackId, bool, bool)> = self
            .mounted
            .iter()
            .filter(|(k, _)| !seen.contains(k))
            .map(|(k, m)| (*k, m.playback, m.spec.finish, m.spec.looped))
            .collect();
        for (key, playback, finish, looped) in gone {
            self.mounted.remove(&key);
            if finish && !looped {
                continue;
            }
            self.stop(playback, 0.0);
            // A one-shot that did not ask to be released is the case
            // `truncated-playback` is about — if it was still running,
            // which only the driver can say.
            if !looped && !finish {
                self.record_stop(playback, key, Why::Removed);
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

    /// F29: the node's removal releases the playback, so a one-shot the
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

    /// F35: the device refuses a play past its 128 voices, and the
    /// playback that never starts never ends — so a tagged node waiting
    /// for `ended` would wait forever. It hears `refused` instead, on the
    /// key it declared, and can tell the two phases apart.
    #[test]
    fn a_refused_tagged_playback_reports_refused_and_warns() {
        let mut a = AudioStore::default();
        let s = sound();
        let k = Key::ROOT.str("chime");
        let p = a.play(
            OriginId::HOST,
            k,
            s,
            PlayOptions::default().tag(Value::str("chime")),
        );
        let (event, warning) = a.refused(p);
        let ev = event.expect("a tagged playback hears the refusal");
        assert_eq!(ev.key, k);
        assert_eq!(
            ev.payload.get("kind").and_then(Value::as_str),
            Some("sound")
        );
        assert_eq!(
            ev.payload.get("phase").and_then(Value::as_str),
            Some("refused"),
            "told apart from the `ended` that will never come"
        );
        assert_eq!(ev.payload.get("tag").and_then(Value::as_str), Some("chime"));
        assert_eq!(
            ev.payload.get("playback").and_then(Value::as_int),
            Some(p.0 as i64)
        );
        assert_eq!(warning.code, crate::diag::PLAYBACK_REFUSED);
        assert_eq!(warning.key, k);

        // Reported once: the refusal is consumed like an end.
        assert!(a.refused(p).0.is_none());
        assert!(a.ended(p).is_none(), "and it can never end afterwards");
    }

    /// An untagged play — a `click_sound`, a bare `Core::play` — has no
    /// view waiting on it, so the warning is the whole report.
    #[test]
    fn a_refused_untagged_playback_is_the_warning_alone() {
        let mut a = AudioStore::default();
        let s = sound();
        let p = a.play(OriginId::HOST, Key::ROOT, s, PlayOptions::default());
        let (event, warning) = a.refused(p);
        assert!(event.is_none(), "nothing asked to hear about this one");
        assert_eq!(warning.code, crate::diag::PLAYBACK_REFUSED);
        assert_eq!(warning.key, Key::ROOT);
    }

    /// An `audio` node's playback survives the node — that is what
    /// `finish` means — so a refusal after the release still has the tag
    /// to report on, and the key the node declared it under.
    #[test]
    fn a_refused_released_playback_still_reports() {
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

        // Gone: released, and the store forgets the mount.
        a.reconcile();
        assert!(a.playback_of(k).is_none());

        let (event, warning) = a.refused(p);
        let ev = event.expect("a released playback still hears the refusal");
        assert_eq!(ev.key, k);
        assert_eq!(
            ev.payload.get("phase").and_then(Value::as_str),
            Some("refused")
        );
        assert_eq!(warning.key, k);
    }

    /// A mounted `audio` node without a tag hangs its warning on the node
    /// rather than the root, so the line names the element that asked.
    #[test]
    fn an_untagged_audio_node_warns_on_its_own_key() {
        let mut a = AudioStore::default();
        let s = sound();
        let k = Key::ROOT.str("bed");
        a.declare(k, OriginId::HOST, AudioSpec::new(s).looped());
        a.reconcile();
        let p = a.playback_of(k).unwrap();

        let (event, warning) = a.refused(p);
        assert!(event.is_none());
        assert_eq!(warning.key, k);
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
