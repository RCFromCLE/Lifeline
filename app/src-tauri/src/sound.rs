//! Short sound cues for moments that matter in play: level up, a new act,
//! walking into a boss arena, a resistance penalty, death, and answers
//! being ready. Synthesized in code (no audio files) and played on their
//! own thread so they work while the windows are hidden.

use std::sync::mpsc::{channel, Sender};
use std::sync::{Mutex, OnceLock};

use tauri::{Emitter, Manager};

use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cue {
    LevelUp,
    NewAct,
    BossArea,
    Penalty,
    Death,
    Ready,
}

impl Cue {
    pub fn name(self) -> &'static str {
        match self {
            Cue::LevelUp => "level_up",
            Cue::NewAct => "new_act",
            Cue::BossArea => "boss_area",
            Cue::Penalty => "penalty",
            Cue::Death => "death",
            Cue::Ready => "ready",
        }
    }

    /// What the HUD pop-up says the sound was.
    pub fn title(self) -> &'static str {
        match self {
            Cue::LevelUp => "Level up",
            Cue::NewAct => "New act",
            Cue::BossArea => "Boss area",
            Cue::Penalty => "Resistance penalty",
            Cue::Death => "Death",
            Cue::Ready => "Ready",
        }
    }

    pub fn from_name(name: &str) -> Option<Cue> {
        Some(match name {
            "level_up" => Cue::LevelUp,
            "new_act" => Cue::NewAct,
            "boss_area" => Cue::BossArea,
            "penalty" => Cue::Penalty,
            "death" => Cue::Death,
            "ready" => Cue::Ready,
            _ => return None,
        })
    }
}

const RATE: u32 = 44_100;

/// One tone: frequency (Hz), start and length (s), loudness, and the
/// overtone mix (bell-like when `bell` is high).
struct Note {
    freq: f32,
    start: f32,
    len: f32,
    amp: f32,
    bell: f32,
}

fn n(freq: f32, start: f32, len: f32, amp: f32, bell: f32) -> Note {
    Note { freq, start, len, amp, bell }
}

/// Renders notes into mono samples with a short attack and an exponential
/// decay, plus a soft limiter.
fn render(notes: &[Note], attack: f32) -> Vec<f32> {
    let total = notes.iter().map(|x| x.start + x.len).fold(0.0, f32::max) + 0.05;
    let mut out = vec![0.0f32; (total * RATE as f32) as usize];
    for note in notes {
        let first = (note.start * RATE as f32) as usize;
        let count = (note.len * RATE as f32) as usize;
        for i in 0..count {
            let t = i as f32 / RATE as f32;
            let env = (t / attack).min(1.0) * (-t * 4.0 / note.len).exp();
            let w = std::f32::consts::TAU * note.freq * t;
            // Bell: inharmonic partials; otherwise warm harmonics.
            let tone = if note.bell > 0.0 {
                w.sin() + note.bell * (0.5 * (w * 2.76).sin() + 0.25 * (w * 5.4).sin())
            } else {
                w.sin() + 0.3 * (w * 2.0).sin() + 0.12 * (w * 3.0).sin()
            };
            if let Some(s) = out.get_mut(first + i) {
                *s += tone * env * note.amp;
            }
        }
    }
    for s in &mut out {
        *s = (*s * 0.9).tanh();
    }
    out
}

fn samples(cue: Cue) -> Vec<f32> {
    match cue {
        // Rising, bright arpeggio.
        Cue::LevelUp => render(
            &[
                n(523.3, 0.00, 0.5, 0.22, 0.5),
                n(659.3, 0.09, 0.5, 0.22, 0.5),
                n(784.0, 0.18, 0.6, 0.22, 0.5),
                n(1046.5, 0.27, 0.9, 0.24, 0.6),
            ],
            0.006,
        ),
        // A swelling major chord that resolves upward.
        Cue::NewAct => render(
            &[
                n(196.0, 0.0, 1.6, 0.18, 0.0),
                n(261.6, 0.0, 1.6, 0.16, 0.0),
                n(329.6, 0.05, 1.6, 0.14, 0.0),
                n(392.0, 0.55, 1.4, 0.16, 0.2),
                n(523.3, 0.6, 1.5, 0.14, 0.3),
            ],
            0.12,
        ),
        // Low, uneasy pair a semitone apart.
        Cue::BossArea => render(
            &[n(110.0, 0.0, 1.8, 0.3, 0.0), n(116.5, 0.15, 1.8, 0.24, 0.0), n(55.0, 0.0, 2.0, 0.22, 0.0)],
            0.25,
        ),
        // Two falling warning tones.
        Cue::Penalty => render(&[n(659.3, 0.0, 0.32, 0.22, 0.1), n(493.9, 0.22, 0.5, 0.24, 0.1)], 0.01),
        // A deep bell, tolled twice.
        Cue::Death => render(&[n(98.0, 0.0, 2.6, 0.4, 1.0), n(98.0, 1.4, 3.0, 0.36, 1.0), n(49.0, 0.0, 3.0, 0.2, 0.0)], 0.004),
        // Soft two-note chime.
        Cue::Ready => render(&[n(784.0, 0.0, 0.35, 0.14, 0.6), n(1174.7, 0.11, 0.6, 0.14, 0.6)], 0.005),
    }
}

fn sender() -> &'static Mutex<Sender<(Cue, f32)>> {
    static SENDER: OnceLock<Mutex<Sender<(Cue, f32)>>> = OnceLock::new();
    SENDER.get_or_init(|| {
        let (tx, rx) = channel::<(Cue, f32)>();
        std::thread::spawn(move || {
            // The output stream must live on this thread.
            let Ok((_stream, handle)) = rodio::OutputStream::try_default() else {
                return;
            };
            for (cue, volume) in rx {
                if let Ok(sink) = rodio::Sink::try_new(&handle) {
                    sink.set_volume(volume);
                    sink.append(rodio::buffer::SamplesBuffer::new(1, RATE, samples(cue)));
                    sink.detach();
                }
            }
        });
        Mutex::new(tx)
    })
}

/// Plays `cue` if sounds are on and this cue is enabled (Settings), with
/// the HUD showing just its name.
#[allow(dead_code)]
pub fn play(app: &tauri::AppHandle, cue: Cue) {
    play_with(app, cue, "");
}

/// Plays `cue` and shows what it means on the HUD for a few seconds
/// (`detail`, e.g. "Level 14", says what happened), so a sound is never a
/// mystery. Nothing shows when the sound is off.
pub fn play_with(app: &tauri::AppHandle, cue: Cue, detail: &str) {
    let state = app.state::<AppState>();
    let (on, volume) = {
        let s = state.settings.lock().unwrap();
        // A cue missing from saved settings falls back to its default.
        let enabled = s
            .sound_cues
            .get(cue.name())
            .copied()
            .or_else(|| crate::state::default_cues().get(cue.name()).copied())
            .unwrap_or(true);
        (s.sound && enabled, s.volume)
    };
    if on {
        let _ = sender().lock().unwrap().send((cue, volume.clamp(0.0, 1.0)));
        let _ = app.emit("sound-cue", serde_json::json!({"cue": cue.name(), "title": cue.title(), "detail": detail}));
    }
}

/// Plays `cue` regardless of the setting (the Settings "test" buttons).
pub fn preview(cue: Cue, volume: f32) {
    let _ = sender().lock().unwrap().send((cue, volume.clamp(0.0, 1.0)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_cue_renders_audible_and_unclipped() {
        for cue in [Cue::LevelUp, Cue::NewAct, Cue::BossArea, Cue::Penalty, Cue::Death, Cue::Ready] {
            let s = samples(cue);
            let peak = s.iter().fold(0.0f32, |m, x| m.max(x.abs()));
            let secs = s.len() as f32 / RATE as f32;
            assert!(peak > 0.05 && peak <= 1.0, "{cue:?} peak {peak}");
            assert!((0.3..6.0).contains(&secs), "{cue:?} lasts {secs}s");
        }
        assert_eq!(Cue::from_name("death"), Some(Cue::Death));
        let defaults = crate::state::default_cues();
        assert!(!defaults["level_up"], "level-up sound is opt-in");
        assert!(!defaults["death"], "death sound is opt-in");
        for cue in [Cue::LevelUp, Cue::NewAct, Cue::BossArea, Cue::Penalty, Cue::Death, Cue::Ready] {
            assert!(defaults.contains_key(cue.name()));
        }
        assert_eq!(Cue::from_name("nope"), None);
    }
}