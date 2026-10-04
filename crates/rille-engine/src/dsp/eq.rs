//! Three-band isolator EQ, as on a DJ mixer.
//!
//! Linkwitz-Riley 4th-order crossovers at [`LOW_MID_HZ`] and [`MID_HIGH_HZ`]. The low band also
//! passes an allpass at the upper crossover so all three bands share the same phase: with every
//! band at unity the EQ is a pure allpass (flat magnitude). Each band's gain is smoothed per
//! sample; turning a band fully down removes it ("kill").

use super::biquad::{BUTTERWORTH_Q, Biquad, Coefficients};
use crate::smooth::Smoother;

pub const LOW_MID_HZ: f64 = 200.0;
pub const MID_HIGH_HZ: f64 = 2000.0;
/// Maximum boost at the right end of an EQ knob.
pub const MAX_BOOST_DB: f32 = 6.0;
const GAIN_SMOOTHING_MS: f32 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Band {
    Low = 0,
    Mid = 1,
    High = 2,
}

/// Knob position (0..1) → linear band gain. 0 = kill, 0.5 = unity, 1 = +6 dB. The cut half is
/// squared so most of the travel is musically useful (-12 dB at a quarter).
pub fn knob_gain(position: f32) -> f32 {
    let p = if position.is_finite() {
        position.clamp(0.0, 1.0)
    } else {
        0.5
    };
    if p <= 0.5 {
        let t = p * 2.0;
        t * t
    } else {
        10f32.powf((p - 0.5) * 2.0 * MAX_BOOST_DB / 20.0)
    }
}

#[derive(Debug, Clone, Copy)]
struct Lr4 {
    a: Biquad,
    b: Biquad,
}

impl Lr4 {
    fn new(c: Coefficients) -> Lr4 {
        Lr4 {
            a: Biquad::new(c),
            b: Biquad::new(c),
        }
    }

    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        self.b.process(self.a.process(x))
    }
}

#[derive(Debug, Clone, Copy)]
struct Channel {
    low_lp: Lr4,
    low_ap: Biquad,
    rest_hp: Lr4,
    mid_lp: Lr4,
    high_hp: Lr4,
}

impl Channel {
    fn new(sample_rate: u32) -> Channel {
        let lp1 = Coefficients::lowpass(LOW_MID_HZ, BUTTERWORTH_Q, sample_rate);
        let hp1 = Coefficients::highpass(LOW_MID_HZ, BUTTERWORTH_Q, sample_rate);
        let lp2 = Coefficients::lowpass(MID_HIGH_HZ, BUTTERWORTH_Q, sample_rate);
        let hp2 = Coefficients::highpass(MID_HIGH_HZ, BUTTERWORTH_Q, sample_rate);
        let ap2 = Coefficients::allpass(MID_HIGH_HZ, BUTTERWORTH_Q, sample_rate);
        Channel {
            low_lp: Lr4::new(lp1),
            low_ap: Biquad::new(ap2),
            rest_hp: Lr4::new(hp1),
            mid_lp: Lr4::new(lp2),
            high_hp: Lr4::new(hp2),
        }
    }

    #[inline]
    fn process(&mut self, x: f32, gains: [f32; 3]) -> f32 {
        let low = self.low_ap.process(self.low_lp.process(x));
        let rest = self.rest_hp.process(x);
        let mid = self.mid_lp.process(rest);
        let high = self.high_hp.process(rest);
        low * gains[0] + mid * gains[1] + high * gains[2]
    }
}

pub struct Eq {
    channels: [Channel; 2],
    gains: [Smoother; 3],
    knobs: [f32; 3],
    kills: [bool; 3],
}

impl Eq {
    pub fn new(sample_rate: u32) -> Eq {
        Eq {
            channels: [Channel::new(sample_rate), Channel::new(sample_rate)],
            gains: [Smoother::new(sample_rate, GAIN_SMOOTHING_MS, 1.0); 3],
            knobs: [0.5; 3],
            kills: [false; 3],
        }
    }

    pub fn knobs(&self) -> [f32; 3] {
        self.knobs
    }

    pub fn kills(&self) -> [bool; 3] {
        self.kills
    }

    pub fn set_knob(&mut self, band: Band, position: f32) {
        self.knobs[band as usize] = if position.is_finite() {
            position.clamp(0.0, 1.0)
        } else {
            0.5
        };
        self.update(band);
    }

    pub fn set_kill(&mut self, band: Band, kill: bool) {
        self.kills[band as usize] = kill;
        self.update(band);
    }

    fn update(&mut self, band: Band) {
        let i = band as usize;
        let target = if self.kills[i] {
            0.0
        } else {
            knob_gain(self.knobs[i])
        };
        self.gains[i].set_target(target);
    }

    /// Process interleaved stereo in place.
    #[inline]
    pub fn process(&mut self, buffer: &mut [f32]) {
        for frame in buffer.as_chunks_mut::<2>().0 {
            let g = [
                self.gains[0].tick(),
                self.gains[1].tick(),
                self.gains[2].tick(),
            ];
            frame[0] = self.channels[0].process(frame[0], g);
            frame[1] = self.channels[1].process(frame[1], g);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::testutil::{gain_db, sine};

    const SR: u32 = 48_000;

    fn measure(eq: &mut Eq, freq: f32) -> f32 {
        let mono = sine(freq, SR, 1.0);
        let mut stereo: Vec<f32> = mono.iter().flat_map(|v| [*v, *v]).collect();
        eq.process(&mut stereo);
        let left: Vec<f32> = stereo.as_chunks::<2>().0.iter().map(|f| f[0]).collect();
        gain_db(&mono, &left)
    }

    #[test]
    fn neutral_eq_is_flat() {
        for f in [40.0, 150.0, 200.0, 700.0, 2000.0, 5000.0, 12_000.0] {
            let mut eq = Eq::new(SR);
            let g = measure(&mut eq, f);
            assert!(g.abs() < 0.3, "{f} Hz: {g:.2} dB");
        }
    }

    #[test]
    fn kill_removes_its_band() {
        for (band, freq) in [(Band::Low, 60.0), (Band::Mid, 630.0), (Band::High, 8000.0)] {
            let mut eq = Eq::new(SR);
            eq.set_kill(band, true);
            let g = measure(&mut eq, freq);
            assert!(g < -30.0, "{band:?} at {freq} Hz: {g:.1} dB");
        }
    }

    #[test]
    fn knob_fully_left_equals_kill() {
        let mut eq = Eq::new(SR);
        eq.set_knob(Band::High, 0.0);
        assert!(measure(&mut eq, 8000.0) < -30.0);
    }

    #[test]
    fn full_boost_is_six_db_in_band() {
        let mut eq = Eq::new(SR);
        eq.set_knob(Band::Mid, 1.0);
        let g = measure(&mut eq, 630.0);
        assert!((g - 6.0).abs() < 0.6, "{g:.2} dB");
    }

    #[test]
    fn other_bands_stay_when_one_is_killed() {
        let mut eq = Eq::new(SR);
        eq.set_kill(Band::Low, true);
        assert!(measure(&mut eq, 8000.0).abs() < 0.5);
    }

    #[test]
    fn knob_curve() {
        assert_eq!(knob_gain(0.0), 0.0);
        assert_eq!(knob_gain(0.5), 1.0);
        assert!((20.0 * knob_gain(1.0).log10() - MAX_BOOST_DB).abs() < 1e-4);
        assert!((20.0 * knob_gain(0.25).log10() + 12.04).abs() < 0.05);
        assert_eq!(knob_gain(f32::NAN), 1.0, "bad input is neutral");
    }

    #[test]
    fn turning_a_band_does_not_click() {
        let mut eq = Eq::new(SR);
        let mut buf = vec![0.5f32; 4800 * 2]; // DC sits in the low band
        eq.process(&mut buf);
        eq.set_kill(Band::Low, true);
        let mut out = vec![0.5f32; 4800 * 2];
        eq.process(&mut out);
        let mut prev = buf[buf.len() - 2];
        for f in out.as_chunks::<2>().0 {
            assert!((f[0] - prev).abs() < 0.01);
            prev = f[0];
        }
    }
}
