/// Encodes mono float samples (-1..1) as a 16-bit PCM WAV file, ready for
/// `Core::add_sound`.
///
/// ```rust
/// let wav = kui_native::audio::wav_pcm16(44_100, &[0.0, 0.5, -0.5]);
/// assert_eq!(&wav[..4], b"RIFF");
/// assert_eq!(wav.len(), 44 + 3 * 2);
/// ```
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

/// A short decaying sine of `hz` lasting `ms`, as a WAV file: a click for
/// examples and tests. `gain` is 0 to 1.
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
