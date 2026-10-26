//! The optional "here I am" blip: a short two-note chime, synthesised once into
//! an in-memory WAV and played with PlaySound. No audio files ship with Shook.

use std::sync::OnceLock;
use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};

const RATE: u32 = 44_100;

fn chime() -> Vec<u8> {
    // C6 then G6, each a soft sine with a quick attack and an exponential tail.
    let notes = [(1046.5f32, 0.0f32), (1568.0, 0.075)];
    let len = (RATE as f32 * 0.32) as usize;
    let mut samples = vec![0f32; len];
    for (freq, at) in notes {
        let start = (at * RATE as f32) as usize;
        for (i, s) in samples.iter_mut().enumerate().skip(start) {
            let t = (i - start) as f32 / RATE as f32;
            let env = (t / 0.004).min(1.0) * (-t * 18.0).exp();
            let tone = (std::f32::consts::TAU * freq * t).sin() + 0.25 * (std::f32::consts::TAU * freq * 2.0 * t).sin();
            *s += tone * env * 0.22;
        }
    }

    let data: Vec<u8> = samples
        .iter()
        .flat_map(|s| ((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).to_le_bytes())
        .collect();
    let mut wav = Vec::with_capacity(44 + data.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&RATE.to_le_bytes());
    wav.extend_from_slice(&(RATE * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
    wav.extend_from_slice(&data);
    wav
}

pub fn play() {
    static WAV: OnceLock<Vec<u8>> = OnceLock::new();
    let wav = WAV.get_or_init(chime);
    unsafe { PlaySoundW(wav.as_ptr().cast(), std::ptr::null_mut(), SND_MEMORY | SND_ASYNC | SND_NODEFAULT) };
}
