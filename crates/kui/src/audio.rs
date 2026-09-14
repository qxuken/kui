//! The audio device behind the core's [`AudioCommand`]s. The core queues
//! commands as data (`Core::take_audio_commands`); the shell hands them
//! here after every input dispatch and every frame, and polls finished
//! playbacks back into the core (`Core::audio_ended`) so tagged ones become
//! `sound` events. It answers the other way too: a `Stop` whose handle was
//! still playing goes back as `Core::audio_truncated`, since whether a
//! sound was still running is the one thing the core cannot see, and a play
//! the device refuses goes back as `Core::audio_refused`: it never starts
//! and so never ends, so the view has to hear about it or wait forever.
//!
//! Backed by kira (cpal underneath) behind the `audio` cargo feature. The
//! device opens lazily on the first play, so an app that never plays
//! anything never starts an audio thread; a device that refuses to open
//! logs once and every later command is dropped — the UI keeps running.
//! Decoded sounds are cached per `SoundId` and dropped on `Unload`.

use kui_core::{AudioCommand, AudioDevice, AudioEnv, PlaybackId, SharedResources};

pub use backend::Audio;

/// A `Stop` that landed on a sound still playing, and how far into the
/// sound (seconds) it was — what `Core::audio_truncated` turns into a
/// `truncated-playback` warning on the node.
pub type Truncated = (PlaybackId, f64);

/// What the device answered back on an [`Audio::apply`] — the two things
/// only it can know. `truncated` is the stops that landed on a sound still
/// playing, `refused` the plays it would not take; the core turns each into
/// a warning on the node that asked, since the driver is key-blind.
#[derive(Default, Debug)]
pub struct Answered {
    pub truncated: Vec<Truncated>,
    pub refused: Vec<PlaybackId>,
}

/// Encodes mono float samples (−1..1) as a 16-bit PCM WAV file — enough to
/// hand a synthesized blip to `Core::add_sound` without shipping assets.
pub fn wav_pcm16(sample_rate: u32, samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // byte rate
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// A short decaying sine — a click / blip for examples and tests.
pub fn blip(sample_rate: u32, hz: f32, ms: f32, gain: f32) -> Vec<u8> {
    let n = (sample_rate as f32 * ms / 1000.0) as usize;
    let samples: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            let env = 1.0 - i as f32 / n as f32;
            (t * hz * std::f32::consts::TAU).sin() * env * env * gain
        })
        .collect();
    wav_pcm16(sample_rate, &samples)
}

#[cfg(feature = "audio")]
mod backend {
    use std::collections::HashMap;
    use std::io::Cursor;
    use std::sync::{Arc, mpsc};
    use std::time::Duration;

    use kira::sound::PlaybackState;
    use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle};
    use kira::{AudioManager, AudioManagerSettings, Decibels, DefaultBackend, Tween};
    use kui_core::SoundId;

    use super::*;

    pub struct Audio {
        device: Device,
        /// Commands that arrived while the device was still opening, in
        /// order, with the registry to decode their sounds from. Applied
        /// the moment it is open: a click sound plays a few ms late
        /// rather than the frame stalling for the open.
        pending: Vec<(AudioCommand, SharedResources)>,
        decoded: HashMap<SoundId, StaticSoundData>,
        playing: HashMap<PlaybackId, StaticSoundHandle>,
        /// Stops applied to a handle that was still playing, drained by
        /// `apply` — including the ones a `flush_pending` produced.
        truncated: Vec<Truncated>,
        /// Plays the device would not take, since the last drain. Buffered
        /// rather than returned inline because a refusal can happen inside
        /// `flush_pending`, which runs from the poll as well as from
        /// `apply`; [`Audio::apply`] is where the driver collects them.
        refused: Vec<PlaybackId>,
    }

    /// The output device, which takes ~90 ms to open on macOS — six frames
    /// — so it is opened on its own thread and never on the loop's. Before
    /// this, the counter's first click stalled for the open (its buttons
    /// carry a click sound) and every later one flew.
    enum Device {
        Closed,
        Opening(mpsc::Receiver<Result<AudioManager<DefaultBackend>, String>>),
        Open(Box<AudioManager<DefaultBackend>>),
        /// The device refused to open; commands are dropped after one
        /// warning.
        Failed,
    }

    impl Default for Audio {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Audio {
        pub fn new() -> Self {
            Audio {
                device: Device::Closed,
                pending: Vec::new(),
                decoded: HashMap::new(),
                playing: HashMap::new(),
                truncated: Vec::new(),
                refused: Vec::new(),
            }
        }

        /// Starts opening the device if nothing has yet. Cheap to call
        /// every frame; the driver calls it once the session holds a
        /// sound, so the device is open by the time a click asks for one.
        pub fn warm(&mut self) {
            if !matches!(self.device, Device::Closed) {
                return;
            }
            let (tx, rx) = mpsc::channel();
            self.device = Device::Opening(rx);
            std::thread::Builder::new()
                .name("kui-audio-open".into())
                .spawn(move || {
                    let opened =
                        AudioManager::<DefaultBackend>::new(AudioManagerSettings::default())
                            .map_err(|e| e.to_string());
                    let _ = tx.send(opened);
                })
                .expect("spawn the audio-open thread");
        }

        /// Whether the device is still opening: commands wait.
        fn opening(&mut self) -> bool {
            let Device::Opening(rx) = &self.device else {
                return false;
            };
            match rx.try_recv() {
                Ok(Ok(m)) => self.device = Device::Open(Box::new(m)),
                Ok(Err(e)) => {
                    eprintln!("kui: audio device unavailable ({e}); sounds are dropped");
                    self.device = Device::Failed;
                }
                Err(mpsc::TryRecvError::Empty) => return true,
                Err(mpsc::TryRecvError::Disconnected) => {
                    eprintln!(
                        "kui: audio device unavailable (the open thread died); sounds are dropped"
                    );
                    self.device = Device::Failed;
                }
            }
            false
        }

        /// The device, once open. Starts the open if nothing has.
        fn manager(&mut self) -> Option<&mut AudioManager<DefaultBackend>> {
            self.warm();
            if self.opening() {
                return None;
            }
            match &mut self.device {
                Device::Open(m) => Some(m),
                _ => None,
            }
        }

        /// Applies what waited for the device, once it is open. Nothing
        /// happens while it is still opening, or before anything waited.
        fn flush_pending(&mut self) {
            if self.pending.is_empty() || self.opening() {
                return;
            }
            let pending = std::mem::take(&mut self.pending);
            for (cmd, resources) in pending {
                self.apply_one(cmd, &resources);
            }
        }

        /// The decoded sound, decoding (and caching) on first use.
        fn decoded(
            &mut self,
            sound: SoundId,
            resources: &SharedResources,
        ) -> Option<StaticSoundData> {
            if let Some(d) = self.decoded.get(&sound) {
                return Some(d.clone());
            }
            let bytes: Arc<[u8]> = resources.sound(sound)?;
            match StaticSoundData::from_cursor(Cursor::new(bytes)) {
                Ok(d) => {
                    self.decoded.insert(sound, d.clone());
                    Some(d)
                }
                Err(e) => {
                    eprintln!("kui: sound failed to decode: {e}");
                    None
                }
            }
        }

        /// Applies queued commands to the device. While it is still
        /// opening they wait, in order, behind whatever waited before.
        /// Returns what the device answered back: the stops that landed on
        /// a sound still playing and the plays it would not take, the way
        /// [`Self::poll_ended`] returns the ones that finished. Both are
        /// buffered rather than produced inline, because either can happen
        /// inside `flush_pending`, which runs from the poll as well as from
        /// here — so a command that waited for the device reports on the
        /// apply that flushes it, a later one than the apply that queued it.
        pub fn apply(&mut self, cmds: Vec<AudioCommand>, resources: &SharedResources) -> Answered {
            if !cmds.is_empty() {
                self.warm();
                self.flush_pending();
                if self.opening() {
                    self.pending
                        .extend(cmds.into_iter().map(|c| (c, resources.clone())));
                } else {
                    for cmd in cmds {
                        self.apply_one(cmd, resources);
                    }
                }
            }
            Answered {
                truncated: std::mem::take(&mut self.truncated),
                refused: std::mem::take(&mut self.refused),
            }
        }

        fn apply_one(&mut self, cmd: AudioCommand, resources: &SharedResources) {
            {
                match cmd {
                    AudioCommand::Play {
                        playback,
                        sound,
                        volume,
                        looped,
                        fade_in_ms,
                    } => {
                        let Some(mut data) = self.decoded(sound, resources) else {
                            // Nothing to play and nothing that will ever
                            // end: the core has to hear so a tagged node
                            // stops waiting.
                            self.refused.push(playback);
                            return;
                        };
                        data = data.volume(db(volume));
                        if looped {
                            data = data.loop_region(..);
                        }
                        if fade_in_ms > 0.0 {
                            data = data.fade_in_tween(tween(fade_in_ms));
                        }
                        // `apply` and `flush_pending` only reach here once
                        // the open has answered, so no device is a device
                        // that failed to open: a machine with no output
                        // (CI, a container, a muted VM). The play is
                        // refused like any other the device would not
                        // take — a view sequenced on `sound ended` must
                        // not hang on it (AR20).
                        let Some(m) = self.manager() else {
                            self.refused.push(playback);
                            return;
                        };
                        match m.play(data) {
                            Ok(h) => {
                                self.playing.insert(playback, h);
                            }
                            // The device's voices are all held (128, with
                            // released playbacks among them). Routed rather
                            // than printed: a stderr line is not something
                            // the view waiting on this sound can hear.
                            Err(_) => self.refused.push(playback),
                        }
                    }
                    AudioCommand::Stop { playback, fade_ms } => {
                        if let Some(mut h) = self.playing.remove(&playback) {
                            // Whether the sound was still running is what
                            // the core is missing; ask before stopping.
                            if h.state() != PlaybackState::Stopped {
                                self.truncated.push((playback, h.position()));
                            }
                            h.stop(tween(fade_ms));
                        }
                    }
                    AudioCommand::SetVolume {
                        playback,
                        volume,
                        tween_ms,
                    } => {
                        if let Some(h) = self.playing.get_mut(&playback) {
                            h.set_volume(db(volume), tween(tween_ms));
                        }
                    }
                    AudioCommand::Pause { playback, fade_ms } => {
                        if let Some(h) = self.playing.get_mut(&playback) {
                            h.pause(tween(fade_ms));
                        }
                    }
                    AudioCommand::Resume { playback, fade_ms } => {
                        if let Some(h) = self.playing.get_mut(&playback) {
                            h.resume(tween(fade_ms));
                        }
                    }
                    AudioCommand::MasterVolume { volume, tween_ms } => {
                        if let Some(m) = self.manager() {
                            m.main_track().set_volume(db(volume), tween(tween_ms));
                        }
                    }
                    AudioCommand::Unload { sound } => {
                        self.decoded.remove(&sound);
                    }
                }
            }
        }

        /// Playbacks that finished on their own since the last poll
        /// (stopped ones were already forgotten by `Stop`). Also where a
        /// command that waited for the device starts, since the driver
        /// polls while anything is active.
        pub fn poll_ended(&mut self) -> Vec<PlaybackId> {
            self.flush_pending();
            let ended: Vec<PlaybackId> = self
                .playing
                .iter()
                .filter(|(_, h)| h.state() == PlaybackState::Stopped)
                .map(|(p, _)| *p)
                .collect();
            for p in &ended {
                self.playing.remove(p);
            }
            ended
        }

        /// Whether the device is open (or opening) and so costing
        /// something. A CoreAudio/WASAPI/ALSA output stream is a real-time
        /// thread that runs whether or not anything is playing — 94 buffer
        /// callbacks a second at the usual 512-frame period — which is the
        /// whole of an idle kui app's CPU once a session holds a sound.
        /// The driver asks so it knows whether there is anything to close.
        pub fn holds_device(&self) -> bool {
            matches!(self.device, Device::Opening(_) | Device::Open(_))
        }

        /// Lets the output device go. The decoded-sound cache stays — it is
        /// the ~90 ms open that has to be paid again, not the decode — so a
        /// re-warm costs nothing a cold start does not. Refuses while
        /// anything is playing or waiting, since that is the device's whole
        /// job; the driver only asks after `active()` has been false for a
        /// while.
        ///
        /// Asks `opening()` first, and that is not a detail: a device the
        /// app warmed but never commanded stays in `Opening` forever —
        /// nothing else on this type calls `opening()` unless a command
        /// flows — with the opened manager sitting live in the channel and
        /// its stream running. That is precisely the case worth closing, so
        /// the state has to be settled before it can be read. Still opening
        /// means not yet closable; the driver asks again.
        pub fn close(&mut self) {
            if self.active() || self.opening() {
                return;
            }
            if matches!(self.device, Device::Open(_)) {
                self.device = Device::Closed;
            }
        }

        /// Whether any playback is live, or waiting on the device to
        /// open — drivers keep polling while so. Both answer buffers count:
        /// a `flush_pending` from the poll can fill either, and the core
        /// only hears them on the next `apply`.
        pub fn active(&self) -> bool {
            !self.playing.is_empty()
                || !self.pending.is_empty()
                || !self.refused.is_empty()
                || !self.truncated.is_empty()
        }

        /// The reading a view gets (`env.audio`): the device's state and
        /// the playbacks started or waiting. The two readers above, as
        /// data — what the driver decides by is what the view can see.
        pub fn env(&self) -> AudioEnv {
            AudioEnv {
                device: match self.device {
                    Device::Closed => AudioDevice::Closed,
                    Device::Opening(_) => AudioDevice::Opening,
                    Device::Open(_) => AudioDevice::Open,
                    Device::Failed => AudioDevice::Failed,
                },
                live: (self.playing.len() + self.pending.len()) as u32,
            }
        }
    }

    /// Linear amplitude to kira's decibels (0 = silence).
    fn db(volume: f32) -> Decibels {
        if volume <= 0.0 {
            Decibels::SILENCE
        } else {
            Decibels(20.0 * volume.log10())
        }
    }

    fn tween(ms: f32) -> Tween {
        Tween {
            duration: Duration::from_secs_f32(ms.max(0.0) / 1000.0),
            ..Default::default()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// An idle device is let go, and a warm that was never commanded
        /// is the case that matters: nothing but `close` calls `opening`,
        /// so a device warmed at launch and never played sits in `Opening`
        /// with a live output stream behind it — 94 buffer callbacks a
        /// second, and the whole of an idle app's CPU. Skipped where there
        /// is no device to open (CI), since there is then nothing to close.
        #[test]
        fn a_warm_device_nothing_used_is_closed_and_reopens() {
            let mut audio = Audio::new();
            audio.warm();
            assert!(audio.holds_device(), "warm holds the device");
            // Let the open land, the way the ~90 ms one does in an app.
            // Ten seconds, not three: right after a windowed smoke round
            // (seventy processes each opening and closing the HAL) the
            // open took over three on a Mac, and a slow open is not a
            // failed one — this test read as red for it (2026-09-14).
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            while matches!(audio.device, Device::Opening(_)) && std::time::Instant::now() < deadline
            {
                std::thread::sleep(Duration::from_millis(20));
                audio.close();
            }
            if matches!(audio.device, Device::Failed) {
                return; // no device on this machine: nothing to close
            }
            assert!(!audio.holds_device(), "an unused device is let go");
            // And the app is not deaf afterwards.
            audio.warm();
            assert!(audio.holds_device(), "a closed device warms again");
        }

        /// A device with something playing is not closed under it.
        #[test]
        fn a_playing_device_is_not_closed() {
            use kui_core::{Core, PlayOptions};
            let mut core = Core::new();
            let s = core.add_sound(super::super::blip(44_100, 220.0, 2_000.0, 0.05));
            core.play(s, PlayOptions::default());
            let mut audio = Audio::new();
            audio.apply(core.take_audio_commands(), &core.resources);
            let deadline = std::time::Instant::now() + Duration::from_secs(3);
            while audio.playing.is_empty() && audio.active() && std::time::Instant::now() < deadline
            {
                std::thread::sleep(Duration::from_millis(20));
                audio.poll_ended();
            }
            if matches!(audio.device, Device::Failed) {
                return; // no device: nothing plays and nothing is held
            }
            audio.close();
            assert!(audio.holds_device(), "a sound mid-flight keeps the device");
        }

        /// AR20: a device that failed to open refuses every play, so the
        /// core hears `refused` and a view sequenced on `sound ended`
        /// does not hang. Before, the play was dropped on the floor and
        /// neither ended nor was refused. Forced rather than found: a
        /// machine with a device cannot fail to open one on demand.
        #[test]
        fn a_device_that_failed_to_open_refuses_a_play() {
            use kui_core::{Core, PlayOptions};
            let mut core = Core::new();
            let s = core.add_sound(super::super::blip(44_100, 660.0, 30.0, 0.1));
            let p = core.play(s, PlayOptions::default());
            let mut audio = Audio::new();
            audio.device = Device::Failed;
            let answered = audio.apply(core.take_audio_commands(), &core.resources);
            assert_eq!(answered.refused, vec![p], "{answered:?}");
            assert!(answered.truncated.is_empty());
            assert!(
                !audio.active(),
                "nothing waits on a device that will not open"
            );
            // And one that waited for the open and then found it failed:
            // reported on the apply that flushes it.
            let mut audio = Audio::new();
            let (_tx, rx) = mpsc::channel();
            audio.device = Device::Opening(rx);
            let p2 = core.play(s, PlayOptions::default());
            let answered = audio.apply(core.take_audio_commands(), &core.resources);
            assert!(answered.refused.is_empty(), "still opening: the play waits");
            drop(_tx); // the open thread "died"
            // The poll is what flushes what waited; the next apply hands
            // the answer back.
            assert!(audio.poll_ended().is_empty());
            let answered = audio.apply(Vec::new(), &core.resources);
            assert_eq!(answered.refused, vec![p2], "{answered:?}");
        }

        /// The decoder needs no device: a synthesized WAV round-trips.
        #[test]
        fn synthesized_wav_decodes() {
            let bytes = super::super::wav_pcm16(8000, &[0.0; 800]);
            let data = StaticSoundData::from_cursor(Cursor::new(bytes)).expect("decodes");
            assert_eq!(data.num_frames(), 800);
            assert_eq!(data.sample_rate, 8000);
        }

        /// Through the real device when the machine has one (the blip
        /// plays and reports ended); without one the backend degrades to
        /// dropping commands, which is the CI case. Either way the first
        /// command never waits for the device: the open is on a thread,
        /// and the play that arrived first starts once it is open.
        #[test]
        fn plays_through_a_device_or_degrades_gracefully() {
            use kui_core::{Core, PlayOptions};
            let mut core = Core::new();
            let s = core.add_sound(super::super::blip(44_100, 660.0, 30.0, 0.1));
            let p = core.play(s, PlayOptions::default());
            let mut audio = Audio::new();
            let t = std::time::Instant::now();
            let answered = audio.apply(core.take_audio_commands(), &core.resources);
            assert!(
                answered.truncated.is_empty() && answered.refused.is_empty(),
                "a play truncates nothing and is not refused: {answered:?}"
            );
            assert!(
                t.elapsed() < Duration::from_millis(20),
                "the first play does not wait for the device: {:?}",
                t.elapsed()
            );
            assert!(audio.active(), "the play waits for the device");
            let deadline = std::time::Instant::now() + Duration::from_secs(3);
            let mut ended = Vec::new();
            while ended.is_empty() && audio.active() && std::time::Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(20));
                ended = audio.poll_ended();
            }
            if matches!(audio.device, Device::Failed) {
                return; // no device: dropped, no panic
            }
            assert_eq!(ended, vec![p], "the 30ms blip reports ended");
            assert!(!audio.active());
        }

        /// F34: a `Stop` that lands on a handle still playing comes back
        /// as a truncation, with where the sound was — the fact the core
        /// cannot see. Through the real device when the machine has one;
        /// without one nothing ever plays, so nothing is truncated, and
        /// the assertion is that it stays quiet rather than guessing.
        #[test]
        fn a_stop_on_a_playing_sound_comes_back_as_a_truncation() {
            use kui_core::{Core, PlayOptions};
            let mut core = Core::new();
            // Two seconds: long enough that the stop below lands inside it
            // whatever the machine is doing.
            let s = core.add_sound(super::super::blip(44_100, 220.0, 2_000.0, 0.05));
            let p = core.play(s, PlayOptions::default());
            let mut audio = Audio::new();
            audio.apply(core.take_audio_commands(), &core.resources);
            // Wait for the device to open and the sound to be running.
            let deadline = std::time::Instant::now() + Duration::from_secs(3);
            while audio.playing.is_empty() && audio.active() && std::time::Instant::now() < deadline
            {
                std::thread::sleep(Duration::from_millis(20));
                audio.poll_ended();
            }
            if matches!(audio.device, Device::Failed) || audio.playing.is_empty() {
                return; // no device: nothing played, so nothing was cut off
            }

            core.stop(p, 0.0);
            let cut = audio
                .apply(core.take_audio_commands(), &core.resources)
                .truncated;
            assert_eq!(cut.len(), 1, "the 2s blip was still playing: {cut:?}");
            assert_eq!(cut[0].0, p);
            assert!(
                cut[0].1 >= 0.0 && cut[0].1 < 2.0,
                "cut inside the sound: {}s",
                cut[0].1
            );
            assert!(
                audio
                    .apply(core.take_audio_commands(), &core.resources)
                    .truncated
                    .is_empty(),
                "reported once"
            );
        }

        #[test]
        fn blip_is_a_valid_wav_of_the_asked_length() {
            let bytes = super::super::blip(44_100, 880.0, 50.0, 0.5);
            let data = StaticSoundData::from_cursor(Cursor::new(bytes)).expect("decodes");
            assert_eq!(data.num_frames(), 2205);
        }
    }
}

#[cfg(not(feature = "audio"))]
mod backend {
    use super::*;

    /// The `audio` feature is off: every command is dropped.
    #[derive(Default)]
    pub struct Audio;

    impl Audio {
        pub fn new() -> Self {
            Audio
        }

        pub fn warm(&mut self) {}

        pub fn holds_device(&self) -> bool {
            false
        }

        pub fn close(&mut self) {}

        pub fn apply(
            &mut self,
            _cmds: Vec<AudioCommand>,
            _resources: &SharedResources,
        ) -> Answered {
            Answered::default()
        }

        pub fn poll_ended(&mut self) -> Vec<PlaybackId> {
            Vec::new()
        }

        pub fn active(&self) -> bool {
            false
        }

        pub fn env(&self) -> AudioEnv {
            AudioEnv::default()
        }
    }
}
