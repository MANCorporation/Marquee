//! Original sound effects and ambient music, synthesised at start-up (no sampled or third-party audio).

use std::f32::consts::TAU;

const RATE: u32 = 44100;

/// 16-bit mono PCM WAV bytes.
fn wav(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut v = Vec::with_capacity(44 + data_len as usize);
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + data_len).to_le_bytes());
    v.extend_from_slice(b"WAVEfmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes()); // PCM
    v.extend_from_slice(&1u16.to_le_bytes()); // mono
    v.extend_from_slice(&RATE.to_le_bytes());
    v.extend_from_slice(&(RATE * 2).to_le_bytes());
    v.extend_from_slice(&2u16.to_le_bytes());
    v.extend_from_slice(&16u16.to_le_bytes());
    v.extend_from_slice(b"data");
    v.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        v.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32000.0) as i16).to_le_bytes());
    }
    v
}

/// One soft "bell" note: a sine with a touch of 2nd harmonic, a short attack and an exponential decay.
fn note(buf: &mut Vec<f32>, start: f32, freq: f32, len: f32, amp: f32, decay: f32) {
    let first = (start * RATE as f32) as usize;
    let n = (len * RATE as f32) as usize;
    if buf.len() < first + n {
        buf.resize(first + n, 0.0);
    }
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        let attack = (t / 0.004).min(1.0);
        let env = attack * (-t * decay).exp();
        let s = (TAU * freq * t).sin() + 0.25 * (TAU * 2.0 * freq * t).sin();
        buf[first + i] += s * env * amp;
    }
}

pub fn sfx(name: &str) -> Vec<u8> {
    let mut b = vec![];
    match name {
        "SelectionChange" => note(&mut b, 0.0, 1318.5, 0.07, 0.32, 55.0),
        "Selection" => {
            note(&mut b, 0.0, 783.99, 0.12, 0.3, 30.0);
            note(&mut b, 0.055, 1174.66, 0.2, 0.3, 22.0);
        }
        "Exit" => {
            note(&mut b, 0.0, 880.0, 0.12, 0.3, 30.0);
            note(&mut b, 0.055, 587.33, 0.2, 0.3, 22.0);
        }
        _ => note(&mut b, 0.0, 196.0, 0.16, 0.45, 26.0), // "Limit": a low thud
    }
    wav(&b)
}

/// A slow, seamless 32-second ambient pad (interleaved stereo) for slideshows and the screen saver.
pub fn ambient() -> Vec<f32> {
    let secs = 32usize;
    let n = secs * RATE as usize;
    let chords: [[f32; 3]; 4] = [[220.0, 261.63, 329.63], [174.61, 261.63, 349.23], [130.81, 261.63, 329.63], [196.0, 246.94, 293.66]];
    let mut out = vec![0.0f32; n * 2];
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        let (mut l, mut r) = (0.0f32, 0.0f32);
        for (c, chord) in chords.iter().enumerate() {
            // Each chord fades in and out over 16 s; neighbours overlap so the sum is constant and loops cleanly.
            let mut dt = t - (c as f32 * 8.0 - 4.0);
            if dt < 0.0 {
                dt += secs as f32;
            }
            if dt >= 16.0 {
                continue;
            }
            let w = (std::f32::consts::PI * dt / 16.0).sin().powi(2);
            for (k, f) in chord.iter().enumerate() {
                let drift = 0.25 + 0.1 * k as f32;
                let tr = 0.8 + 0.2 * (TAU * (0.07 + 0.02 * k as f32) * t).sin();
                l += w * tr * ((TAU * (f - drift) * t).sin() + 0.3 * (TAU * 2.0 * f * t).sin());
                r += w * tr * ((TAU * (f + drift) * t).sin() + 0.3 * (TAU * 2.0 * f * t + 1.0).sin());
            }
        }
        out[i * 2] = l * 0.045;
        out[i * 2 + 1] = r * 0.045;
    }
    out
}

/// A soft, abstract picture for the screen saver when there are no photos: blurred colour blobs.
pub fn card(seed: u32, w: u32, h: u32) -> Vec<u8> {
    // GNOME palette
    const PAL: [[f32; 3]; 10] = [
        [53.0, 132.0, 228.0],
        [145.0, 65.0, 172.0],
        [255.0, 120.0, 0.0],
        [46.0, 194.0, 126.0],
        [246.0, 211.0, 45.0],
        [224.0, 27.0, 36.0],
        [98.0, 160.0, 234.0],
        [192.0, 97.0, 203.0],
        [255.0, 163.0, 72.0],
        [87.0, 227.0, 137.0],
    ];
    let mut s = seed.wrapping_mul(2654435761).wrapping_add(12345);
    let mut rnd = move || {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        (s % 10000) as f32 / 10000.0
    };
    let base = PAL[(rnd() * 10.0) as usize % 10];
    let blobs: Vec<(f32, f32, f32, [f32; 3])> = (0..4).map(|_| (rnd(), rnd(), 0.18 + rnd() * 0.3, PAL[(rnd() * 10.0) as usize % 10])).collect();
    let mut data = vec![255u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
            let shade = 0.35 + 0.35 * (1.0 - fy);
            let mut c = [base[0] * shade, base[1] * shade, base[2] * shade];
            for (bx, by, br, bc) in &blobs {
                let d2 = ((fx - bx) * w as f32 / h as f32).powi(2) + (fy - by).powi(2);
                let k = (-d2 / (br * br)).exp() * 0.85;
                for i in 0..3 {
                    c[i] = c[i] * (1.0 - k) + bc[i] * k;
                }
            }
            let o = ((y * w + x) * 4) as usize;
            for i in 0..3 {
                data[o + i] = c[i].clamp(0.0, 255.0) as u8;
            }
        }
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effects_decode_and_are_audible() {
        for n in ["Selection", "SelectionChange", "Exit", "Limit"] {
            let bytes = sfx(n);
            let dec = rodio::Decoder::new(std::io::Cursor::new(bytes)).expect("decodes");
            let samples: Vec<f32> = dec.collect();
            let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            assert!(samples.len() > 1000 && peak > 0.05, "{n}: len {} peak {peak}", samples.len());
        }
    }

    #[test]
    fn ambient_loops_without_a_jump() {
        let a = ambient();
        let n = a.len() / 2;
        // First and last frames should be close in level (seamless loop).
        assert!((a[0] - a[(n - 1) * 2]).abs() < 0.05);
    }
}
