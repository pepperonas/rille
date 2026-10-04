//! Second-order IIR section (transposed direct form II) with RBJ cookbook designs.

use std::f64::consts::PI;

/// Q of a Butterworth second-order section; two of them in series give Linkwitz-Riley 4th order.
pub const BUTTERWORTH_Q: f64 = std::f64::consts::FRAC_1_SQRT_2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coefficients {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

impl Coefficients {
    fn normalised(b: [f64; 3], a: [f64; 3]) -> Coefficients {
        let a0 = a[0];
        Coefficients {
            b0: (b[0] / a0) as f32,
            b1: (b[1] / a0) as f32,
            b2: (b[2] / a0) as f32,
            a1: (a[1] / a0) as f32,
            a2: (a[2] / a0) as f32,
        }
    }

    fn omega(frequency: f64, sample_rate: u32) -> (f64, f64) {
        let w = 2.0 * PI * frequency / f64::from(sample_rate.max(1));
        (w.cos(), w.sin())
    }

    pub fn lowpass(frequency: f64, q: f64, sample_rate: u32) -> Coefficients {
        let (cos, sin) = Self::omega(frequency, sample_rate);
        let alpha = sin / (2.0 * q);
        Self::normalised(
            [(1.0 - cos) / 2.0, 1.0 - cos, (1.0 - cos) / 2.0],
            [1.0 + alpha, -2.0 * cos, 1.0 - alpha],
        )
    }

    pub fn highpass(frequency: f64, q: f64, sample_rate: u32) -> Coefficients {
        let (cos, sin) = Self::omega(frequency, sample_rate);
        let alpha = sin / (2.0 * q);
        Self::normalised(
            [(1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0],
            [1.0 + alpha, -2.0 * cos, 1.0 - alpha],
        )
    }

    pub fn allpass(frequency: f64, q: f64, sample_rate: u32) -> Coefficients {
        let (cos, sin) = Self::omega(frequency, sample_rate);
        let alpha = sin / (2.0 * q);
        Self::normalised(
            [1.0 - alpha, -2.0 * cos, 1.0 + alpha],
            [1.0 + alpha, -2.0 * cos, 1.0 - alpha],
        )
    }
}

/// One biquad with its own state (one channel).
#[derive(Debug, Clone, Copy)]
pub struct Biquad {
    c: Coefficients,
    z1: f32,
    z2: f32,
}

/// Values below this are flushed to zero so feedback paths never decay into denormals.
const DENORMAL: f32 = 1e-20;

impl Biquad {
    pub fn new(c: Coefficients) -> Biquad {
        Biquad {
            c,
            z1: 0.0,
            z2: 0.0,
        }
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let c = &self.c;
        let y = c.b0 * x + self.z1;
        self.z1 = c.b1 * x - c.a1 * y + self.z2;
        self.z2 = c.b2 * x - c.a2 * y;
        if self.z1.abs() < DENORMAL {
            self.z1 = 0.0;
        }
        if self.z2.abs() < DENORMAL {
            self.z2 = 0.0;
        }
        y
    }

    pub fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::testutil::{gain_db, sine};

    const SR: u32 = 48_000;

    fn response(c: Coefficients, freq: f32) -> f32 {
        let mut f = Biquad::new(c);
        let input = sine(freq, SR, 1.0);
        let output: Vec<f32> = input.iter().map(|x| f.process(*x)).collect();
        gain_db(&input, &output)
    }

    #[test]
    fn butterworth_lowpass_is_minus_three_db_at_cutoff() {
        let c = Coefficients::lowpass(1000.0, BUTTERWORTH_Q, SR);
        assert!((response(c, 1000.0) + 3.01).abs() < 0.2);
        assert!(response(c, 100.0).abs() < 0.1);
        assert!(response(c, 10_000.0) < -35.0);
    }

    #[test]
    fn highpass_mirrors_lowpass() {
        let c = Coefficients::highpass(1000.0, BUTTERWORTH_Q, SR);
        assert!((response(c, 1000.0) + 3.01).abs() < 0.2);
        assert!(response(c, 10_000.0).abs() < 0.1);
        assert!(response(c, 100.0) < -35.0);
    }

    #[test]
    fn allpass_keeps_magnitude() {
        let c = Coefficients::allpass(2000.0, BUTTERWORTH_Q, SR);
        for f in [50.0, 500.0, 2000.0, 8000.0] {
            assert!(response(c, f).abs() < 0.05, "{f} Hz");
        }
    }

    #[test]
    fn silence_decays_to_true_zero() {
        let mut f = Biquad::new(Coefficients::lowpass(200.0, BUTTERWORTH_Q, SR));
        f.process(1.0);
        let mut last = 1.0;
        for _ in 0..SR {
            last = f.process(0.0);
        }
        assert_eq!(last, 0.0);
    }
}
