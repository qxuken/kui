//! The audio device behind the core's [`AudioCommand`]s. The core queues
//! commands as data (`Core::take_audio_commands`); the shell hands them
//! here after every input dispatch and every frame, and polls finished
//! playbacks back into the core (`Core::audio_ended`) so tagged ones become
//! `sound` events.
//!
//! Backed by kira (cpal underneath) behind the `audio` cargo feature. The
//! device opens lazily on the first play, so an app that never plays
//! anything never starts an audio thread; a device that refuses to open
//! logs once and every later command is dropped — the UI keeps running.
//! Decoded sounds are cached per `SoundId` and dropped on `Unload`.

use kui_core::{AudioCommand, PlaybackId, Resources};

pub use backend::Audio;

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
    use std::sync::Arc;
    use std::time::Duration;

    use kira::sound::PlaybackState;
    use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle};
    use kira::{AudioManager, AudioManagerSettings, Decibels, DefaultBackend, Tween};
    use kui_core::SoundId;

    use super::*;

    pub struct Audio {
        manager: Option<AudioManager<DefaultBackend>>,
        /// The device refused to open; commands are dropped after one
        /// warning.
        failed: bool,
        decoded: HashMap<SoundId, StaticSoundData>,
        playing: HashMap<PlaybackId, StaticSoundHandle>,
    }

    impl Default for Audio {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Audio {
        pub fn new() -> Self {
            Audio {
                manager: None,
                failed: false,
                decoded: HashMap::new(),
                playing: HashMap::new(),
            }
        }

        /// The device, opened on first use.
        fn manager(&mut self) -> Option<&mut AudioManager<DefaultBackend>> {
            if self.manager.is_none() && !self.failed {
                match AudioManager::<DefaultBackend>::new(AudioManagerSettings::default()) {
                    Ok(m) => self.manager = Some(m),
                    Err(e) => {
                        eprintln!("kui: audio device unavailable ({e}); sounds are dropped");
                        self.failed = true;
                    }
                }
            }
            self.manager.as_mut()
        }

        /// The decoded sound, decoding (and caching) on first use.
        fn decoded(&mut self, sound: SoundId, resources: &Resources) -> Option<StaticSoundData> {
            if let Some(d) = self.decoded.get(&sound) {
                return Some(d.clone());
            }
            let bytes: Arc<[u8]> = resources.sound(sound)?.clone();
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

        /// Applies queued commands to the device.
        pub fn apply(&mut self, cmds: Vec<AudioCommand>, resources: &Resources) {
            for cmd in cmds {
                match cmd {
                    AudioCommand::Play {
                        playback,
                        sound,
                        volume,
                        looped,
                        fade_in_ms,
                    } => {
                        let Some(mut data) = self.decoded(sound, resources) else {
                            continue;
                        };
                        data = data.volume(db(volume));
                        if looped {
                            data = data.loop_region(..);
                        }
                        if fade_in_ms > 0.0 {
                            data = data.fade_in_tween(tween(fade_in_ms));
                        }
                        let Some(m) = self.manager() else {
                            continue;
                        };
                        match m.play(data) {
                            Ok(h) => {
                                self.playing.insert(playback, h);
                            }
                            Err(e) => eprintln!("kui: play failed: {e}"),
                        }
                    }
                    AudioCommand::Stop { playback, fade_ms } => {
                        if let Some(mut h) = self.playing.remove(&playback) {
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
        /// (stopped ones were already forgotten by `Stop`).
        pub fn poll_ended(&mut self) -> Vec<PlaybackId> {
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

        /// Whether any playback is live — drivers keep polling while so.
        pub fn active(&self) -> bool {
            !self.playing.is_empty()
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
        /// dropping commands, which is the CI case.
        #[test]
        fn plays_through_a_device_or_degrades_gracefully() {
            use kui_core::{Core, PlayOptions};
            let mut core = Core::new();
            let s = core.add_sound(super::super::blip(44_100, 660.0, 30.0, 0.1));
            let p = core.play(s, PlayOptions::default());
            let mut audio = Audio::new();
            audio.apply(core.take_audio_commands(), &core.resources);
            if !audio.active() {
                return; // no device: dropped, no panic
            }
            let deadline = std::time::Instant::now() + Duration::from_secs(3);
            let mut ended = Vec::new();
            while ended.is_empty() && std::time::Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(20));
                ended = audio.poll_ended();
            }
            assert_eq!(ended, vec![p], "the 30ms blip reports ended");
            assert!(!audio.active());
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

        pub fn apply(&mut self, _cmds: Vec<AudioCommand>, _resources: &Resources) {}

        pub fn poll_ended(&mut self) -> Vec<PlaybackId> {
            Vec::new()
        }

        pub fn active(&self) -> bool {
            false
        }
    }
}
