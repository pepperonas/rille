//! Bipolar channel filter (the DDJ-200's colour FX knob): left of centre a low-pass closes,
//! right of centre a high-pass opens, the centre is a true bypass.
//!
//! Topology-preserving-transform state-variable filter (Zavalishin/Simper): stable under fast
//! modulation, so the cutoff can follow the knob without zipper or blow-ups.

use std::f32::consts::PI;

use crate::smooth::Smoother;

/// Half-width of the neutral zone around the centre.
pub const DEAD_ZONE: f32 = 0.03;
/// Width over which the filter fades in after leaving the neutral zone.
const FADE_ZONE: f32 = 0.05;
const LP_FROM_HZ: f32 = 20_000.0;
const LP_TO_HZ: f32 = 80.0;
const HP_FROM_HZ: f32 = 20.0;
const HP_TO_HZ: f32 = 8_000.0;
/// Mild resonance, never self-oscillating.
const Q: f32 = 0.9;
/// Coefficients are recomputed this often (samples) from the smoothed knob position.
const SUB_BLOCK: usize = 16;
const POSITION_SMOOTHING_MS: f32 = 15.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    LowPass,
    HighPass,
}

/// What the filter does at a knob position: mode, cutoff and wet amount (0 = bypass).
pub fn shape(position: f32) -> (Mode, f32, f32) {
    let p = if position.is_finite() {
        position.clamp(0.0, 1.0)
    } else {
        0.5
    };
    let offset = p - 0.5;
    let wet = ((offset.abs() - DEAD_ZONE) / FADE_ZONE).clamp(0.0, 1.0);
    // Travel beyond the dead zone, 0..1.
    let t = ((offset.abs() - DEAD_ZONE) / (0.5 - DEAD_ZONE)).clamp(0.0, 1.0);
    if offset < 0.0 {
        (
            Mode::LowPass,
            LP_FROM_HZ * (LP_TO_HZ / LP_FROM_HZ).powf(t),
            wet,
        )
    } else {
        (
            Mode::HighPass,
            HP_FROM_HZ * (HP_TO_HZ / HP_FROM_HZ).powf(t),
            wet,
        )
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Svf {
    ic1: f32,
    ic2: f32,
}

#[derive(Debug, Clone, Copy)]
struct SvfCoefficients {
    a1: f32,
    a2: f32,
    a3: f32,
    k: f32,
}

impl SvfCoefficients {
    fn new(cutoff: f32, sample_rate: u32) -> SvfCoefficients {
        let nyquist_safe = cutoff.min(sample_rate as f32 * 0.45);
        let g = (PI * nyquist_safe / sample_rate as f32).tan();
        let k = 1.0 / Q;
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        SvfCoefficients { a1, a2, a3, k }
    }
}

impl Svf {
    #[inline]
    fn process(&mut self, x: f32, c: &SvfCoefficients, mode: Mode) -> f32 {
        let v3 = x - self.ic2;
        let v1 = c.a1 * self.ic1 + c.a2 * v3;
        let v2 = self.ic2 + c.a2 * self.ic1 + c.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        if self.ic1.abs() < 1e-20 {
            self.ic1 = 0.0;
        }
        if self.ic2.abs() < 1e-20 {
            self.ic2 = 0.0;
        }
        match mode {
            Mode::LowPass => v2,
            Mode::HighPass => x - c.k * v1 - v2,
        }
    }
}

pub struct BipolarFilter {
    sample_rate: u32,
    position: Smoother,
    channels: [Svf; 2],
    wet: f32,
}

impl BipolarFilter {
    pub fn new(sample_rate: u32) -> BipolarFilter {
        BipolarFilter {
            sample_rate,
            position: Smoother::new(sample_rate, POSITION_SMOOTHING_MS, 0.5),
            channels: [Svf::default(); 2],
            wet: 0.0,
        }
    }

    pub fn position(&self) -> f32 {
        self.position.target()
    }

    pub fn set_position(&mut self, position: f32) {
        self.position.set_target(if position.is_finite() {
            position.clamp(0.0, 1.0)
        } else {
            0.5
        });
    }

    /// Process interleaved stereo in place.
    pub fn process(&mut self, buffer: &mut [f32]) {
        for chunk in buffer.chunks_mut(SUB_BLOCK * 2) {
            let frames = chunk.len() / 2;
            let mut p = self.position.current();
            for _ in 0..frames {
                p = self.position.tick();
            }
            let (mode, cutoff, wet_target) = shape(p);
            if wet_target == 0.0 && self.wet == 0.0 {
                // True bypass; keep the state quiet so re-entering starts cleanly.
                self.channels = [Svf::default(); 2];
                continue;
            }
            let c = SvfCoefficients::new(cutoff, self.sample_rate);
            let step = (wet_target - self.wet) / frames.max(1) as f32;
            for frame in chunk.as_chunks_mut::<2>().0 {
                self.wet += step;
                for (ch, sample) in frame.iter_mut().enumerate() {
                    let filtered = self.channels[ch].process(*sample, &c, mode);
                    *sample += self.wet * (filtered - *sample);
                }
            }
            self.wet = wet_target;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::testutil::{gain_db, sine};

    const SR: u32 = 48_000;

    fn measure(position: f32, freq: f32) -> f32 {
        let mut f = BipolarFilter::new(SR);
        f.set_position(position);
        let mono = sine(freq, SR, 1.0);
        let mut stereo: Vec<f32> = mono.iter().flat_map(|v| [*v, *v]).collect();
        f.process(&mut stereo);
        let left: Vec<f32> = stereo.as_chunks::<2>().0.iter().map(|f| f[0]).collect();
        gain_db(&mono, &left)
    }

    #[test]
    fn centre_is_bit_exact_bypass() {
        let mut f = BipolarFilter::new(SR);
        f.set_position(0.51);
        let input: Vec<f32> = sine(440.0, SR, 0.2)
            .iter()
            .flat_map(|v| [*v, -*v])
            .collect();
        let mut out = input.clone();
        f.process(&mut out);
        assert_eq!(out, input);
    }

    #[test]
    fn low_pass_side_cuts_highs() {
        assert!(measure(0.1, 5000.0) < -20.0);
        assert!(measure(0.1, 60.0).abs() < 1.0);
    }

    #[test]
    fn high_pass_side_cuts_lows() {
        assert!(measure(0.9, 100.0) < -20.0);
        assert!(measure(0.9, 15_000.0).abs() < 1.5);
    }

    #[test]
    fn shape_is_continuous_and_neutral_in_the_middle() {
        assert_eq!(shape(0.5).2, 0.0);
        assert_eq!(shape(0.5 + DEAD_ZONE).2, 0.0);
        assert!(shape(0.0).1 <= LP_TO_HZ + 1.0);
        assert!(shape(1.0).1 >= HP_TO_HZ - 1.0);
        assert_eq!(shape(f32::NAN).2, 0.0);
    }

    #[test]
    fn sweeping_wildly_stays_finite_and_smooth() {
        let mut f = BipolarFilter::new(SR);
        let mut buf: Vec<f32> = sine(1000.0, SR, 2.0)
            .iter()
            .flat_map(|v| [*v, *v])
            .collect();
        for (i, chunk) in buf.chunks_mut(512).enumerate() {
            f.set_position(if i % 2 == 0 { 0.0 } else { 1.0 });
            f.process(chunk);
        }
        assert!(buf.iter().all(|v| v.is_finite() && v.abs() < 4.0));
    }
}
