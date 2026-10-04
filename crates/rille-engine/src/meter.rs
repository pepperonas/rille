//! Peak meter with a fixed fall-back rate, evaluated per block.

/// Peak hold that decays by `FALL_DB_PER_SEC`. Values are linear amplitude.
#[derive(Debug, Clone, Copy)]
pub struct PeakMeter {
    value: [f32; 2],
    decay_per_frame: f32,
}

impl PeakMeter {
    pub const FALL_DB_PER_SEC: f32 = 20.0;

    pub fn new(sample_rate: u32) -> PeakMeter {
        let per_frame_db = Self::FALL_DB_PER_SEC / sample_rate.max(1) as f32;
        PeakMeter {
            value: [0.0; 2],
            decay_per_frame: 10f32.powf(-per_frame_db / 20.0),
        }
    }

    /// Feed one block's peak (already computed by the mixer) covering `frames` frames.
    #[inline]
    pub fn update(&mut self, block_peak: [f32; 2], frames: usize) {
        let decay = self.decay_per_frame.powi(frames as i32);
        for (v, p) in self.value.iter_mut().zip(block_peak) {
            let fallen = *v * decay;
            *v = if p > fallen {
                p
            } else if fallen < 1e-6 {
                0.0
            } else {
                fallen
            };
        }
    }

    pub fn value(&self) -> [f32; 2] {
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rises_instantly() {
        let mut m = PeakMeter::new(48_000);
        m.update([0.5, 0.25], 256);
        assert_eq!(m.value(), [0.5, 0.25]);
    }

    #[test]
    fn falls_twenty_db_per_second() {
        let mut m = PeakMeter::new(48_000);
        m.update([1.0, 1.0], 1);
        m.update([0.0, 0.0], 48_000);
        let db = 20.0 * m.value()[0].log10();
        assert!((db + 20.0).abs() < 0.05, "{db}");
    }

    #[test]
    fn reaches_true_zero() {
        let mut m = PeakMeter::new(48_000);
        m.update([1.0, 1.0], 1);
        for _ in 0..1000 {
            m.update([0.0, 0.0], 4800);
        }
        assert_eq!(m.value(), [0.0, 0.0]);
    }
}
