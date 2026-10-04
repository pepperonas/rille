//! Synthetic test tracks with known tempo and beat positions (tests only).

use std::f32::consts::TAU;

pub const RATE: u32 = 44_100;

/// Deterministic noise in −1..1.
pub struct Noise(u32);

impl Noise {
    pub fn new(seed: u32) -> Noise {
        Noise(seed.max(1))
    }

    pub fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 / (1u32 << 23) as f32 - 1.0
    }
}

/// What to put on the beats.
#[derive(Clone, Copy)]
pub struct Pattern {
    pub bpm: f64,
    /// Time of the first kick in seconds.
    pub first: f64,
    pub seconds: f64,
    /// Hi-hats between the beats; `Some(fraction)` places them at that fraction of the beat
    /// (0.5 straight, 0.66 swung).
    pub hats: Option<f64>,
    /// Snare on beats 2 and 4.
    pub snare: bool,
    /// Silence from beat `start` for `len` beats (a breakdown).
    pub gap: Option<(usize, usize)>,
    pub hat_level: f32,
    /// Hi-hat decay in seconds (closed ~0.025, open ~0.12).
    pub hat_decay: f32,
    /// Tempo wobble: beat length varies by up to this share (a live drummer) …
    pub drift: f64,
    /// … in waves of this many beats.
    pub drift_beats: f64,
}

impl Pattern {
    pub fn kicks(bpm: f64, seconds: f64) -> Pattern {
        Pattern {
            bpm,
            first: 0.123,
            seconds,
            hats: None,
            snare: false,
            gap: None,
            hat_level: 0.2,
            hat_decay: 0.025,
            drift: 0.0,
            drift_beats: 32.0,
        }
    }

    pub fn render(&self) -> Vec<f32> {
        let n = (self.seconds * f64::from(RATE)) as usize;
        let mut out = vec![0.0f32; n];
        let mut noise = Noise::new(7);
        let period = 60.0 / self.bpm;
        let mut beat = 0usize;
        let mut t = self.first;
        loop {
            if t >= self.seconds {
                break;
            }
            let this_period = period
                * (1.0
                    + self.drift * (beat as f64 * std::f64::consts::TAU / self.drift_beats).sin());
            let silent = self.gap.is_some_and(|(s, l)| beat >= s && beat < s + l);
            if !silent {
                kick(&mut out, t);
                if self.snare && beat % 2 == 1 {
                    burst(&mut out, t, 0.35, 0.12, &mut noise);
                }
                if let Some(f) = self.hats {
                    hat(
                        &mut out,
                        t + f * this_period,
                        self.hat_level,
                        self.hat_decay,
                        &mut noise,
                    );
                }
            }
            beat += 1;
            t += this_period;
        }
        out
    }
}

fn at(t: f64) -> usize {
    (t * f64::from(RATE)).round() as usize
}

/// A kick: decaying 55 Hz sine with a short click on top.
fn kick(out: &mut [f32], t: f64) {
    let start = at(t);
    for i in 0..(RATE as usize / 5) {
        let Some(s) = out.get_mut(start + i) else {
            break;
        };
        let x = i as f32 / RATE as f32;
        let body = (x * 55.0 * TAU).sin() * (-x / 0.08).exp();
        let click = if i < 40 {
            0.5 * (1.0 - i as f32 / 40.0)
        } else {
            0.0
        };
        *s += 0.8 * body + click;
    }
}

/// A hi-hat: noise high-passed by double differencing, like the real thing nearly free of
/// low frequencies (white noise would carry as much below 200 Hz as a kick).
fn hat(out: &mut [f32], t: f64, level: f32, decay: f32, noise: &mut Noise) {
    let start = at(t);
    let (mut p1, mut p2) = (0.0f32, 0.0f32);
    for i in 0..(RATE as usize / 4) {
        let Some(s) = out.get_mut(start + i) else {
            break;
        };
        let n = noise.next();
        let high = 0.5 * (n - 2.0 * p1 + p2);
        (p2, p1) = (p1, n);
        let x = i as f32 / RATE as f32;
        *s += level * high * (-x / decay).exp();
    }
}

/// A noise burst (snare).
fn burst(out: &mut [f32], t: f64, level: f32, decay: f32, noise: &mut Noise) {
    let start = at(t);
    for i in 0..(RATE as usize / 4) {
        let Some(s) = out.get_mut(start + i) else {
            break;
        };
        let x = i as f32 / RATE as f32;
        *s += level * noise.next() * (-x / decay).exp();
    }
}
