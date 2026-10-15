//! The "right passive" chime: a short two-note rise, made once in memory as a
//! WAV and played with Windows' own PlaySound (no audio library needed).

use std::sync::OnceLock;

const RATE: u32 = 44_100;

/// Plays the chime without waiting for it to finish.
pub fn play() {
    static WAV: OnceLock<Vec<u8>> = OnceLock::new();
    let wav = WAV.get_or_init(chime_wav);
    play_wav(wav);
}

#[cfg(windows)]
fn play_wav(wav: &'static [u8]) {
    use windows::core::PCWSTR;
    use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
    // SND_MEMORY reads the sound from this buffer while it plays, which is
    // why it lives for the whole run (`'static`).
    unsafe {
        let _ = PlaySoundW(PCWSTR(wav.as_ptr().cast()), None, SND_MEMORY | SND_ASYNC | SND_NODEFAULT);
    }
}

#[cfg(not(windows))]
fn play_wav(_wav: &'static [u8]) {}

/// Two soft bell notes a fifth apart (A5 then E6), each fading out.
fn chime_wav() -> Vec<u8> {
    let note = |freq: f32, start: f32, len: f32, t: f32| -> f32 {
        let t = t - start;
        if !(0.0..len).contains(&t) {
            return 0.0;
        }
        let attack = (t / 0.006).min(1.0);
        let decay = (-t * 9.0).exp();
        let tone = (std::f32::consts::TAU * freq * t).sin() + 0.25 * (std::f32::consts::TAU * freq * 2.0 * t).sin();
        tone * attack * decay
    };
    let seconds = 0.55;
    let samples: Vec<i16> = (0..(RATE as f32 * seconds) as usize)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let v = 0.32 * (note(880.0, 0.0, 0.45, t) + note(1318.5, 0.09, 0.46, t));
            (v.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
        })
        .collect();
    wav(&samples)
}

/// 16-bit mono PCM samples as a WAV file.
fn wav(samples: &[i16]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes()); // bytes per second
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chime_is_a_short_audible_unclipped_wav() {
        let w = chime_wav();
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(&w[8..16], b"WAVEfmt ");
        assert_eq!(u32::from_le_bytes(w[4..8].try_into().unwrap()) as usize, w.len() - 8);
        let samples: Vec<i16> = w[44..].chunks(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
        let peak = samples.iter().map(|s| s.unsigned_abs()).max().unwrap();
        assert!(peak > 6_000, "audible: peak {peak}");
        assert!(peak < i16::MAX as u16, "never clips");
        let seconds = samples.len() as f32 / RATE as f32;
        assert!((0.4..0.8).contains(&seconds), "short: {seconds}s");
    }
}
