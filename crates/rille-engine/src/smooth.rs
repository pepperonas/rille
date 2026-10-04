//! Parameter smoothing. Every user-facing level goes through a [`Smoother`] so that fader and
//! knob moves never produce zipper noise.

/// One-pole low-pass on a control value, time-constant based (independent of block size).
#[derive(Debug, Clone, Copy)]
pub struct Smoother {
    current: f32,
    target: f32,
    coeff: f32,
}

impl Smoother {
    /// `time_ms` is the time constant τ: after τ the value has covered ~63 % of a step.
    pub fn new(sample_rate: u32, time_ms: f32, initial: f32) -> Smoother {
        Smoother {
            current: initial,
            target: initial,
            coeff: Self::coefficient(sample_rate, time_ms),
        }
    }

    /// Change the time constant (e.g. fast while scratching, slower for tempo moves).
    pub fn set_time(&mut self, sample_rate: u32, time_ms: f32) {
        self.coeff = Self::coefficient(sample_rate, time_ms);
    }

    fn coefficient(sample_rate: u32, time_ms: f32) -> f32 {
        let samples = (time_ms / 1000.0) * sample_rate as f32;
        if samples <= 1.0 {
            1.0
        } else {
            1.0 - (-1.0 / samples).exp()
        }
    }

    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    pub fn target(&self) -> f32 {
        self.target
    }

    pub fn current(&self) -> f32 {
        self.current
    }

    /// Jump straight to the target (used on load, never for audible parameters).
    pub fn snap(&mut self) {
        self.current = self.target;
    }

    #[inline]
    pub fn tick(&mut self) -> f32 {
        let delta = self.target - self.current;
        // Settle exactly once the remaining distance is inaudible; avoids denormals too.
        if delta.abs() < 1e-6 {
            self.current = self.target;
        } else {
            self.current += self.coeff * delta;
        }
        self.current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_has_no_zipper() {
        let mut s = Smoother::new(48_000, 10.0, 0.0);
        s.set_target(1.0);
        let mut prev = s.current();
        for _ in 0..48_000 {
            let v = s.tick();
            assert!((v - prev).abs() < 0.01, "jump of {}", v - prev);
            prev = v;
        }
    }

    #[test]
    fn settles_after_five_tau() {
        let mut s = Smoother::new(48_000, 10.0, 0.0);
        s.set_target(1.0);
        for _ in 0..(5 * 480) {
            s.tick();
        }
        assert!((s.current() - 1.0).abs() < 0.01);
    }

    #[test]
    fn reaches_target_exactly_eventually() {
        let mut s = Smoother::new(48_000, 1.0, 0.0);
        s.set_target(0.25);
        for _ in 0..48_000 {
            s.tick();
        }
        assert_eq!(s.current(), 0.25);
    }

    #[test]
    fn snap_jumps() {
        let mut s = Smoother::new(48_000, 10.0, 0.0);
        s.set_target(0.7);
        s.snap();
        assert_eq!(s.tick(), 0.7);
    }

    #[test]
    fn zero_time_is_immediate() {
        let mut s = Smoother::new(48_000, 0.0, 0.0);
        s.set_target(1.0);
        assert_eq!(s.tick(), 1.0);
    }
}
