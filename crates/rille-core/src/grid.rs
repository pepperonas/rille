//! Constant-tempo beatgrid.

/// Beats at `first_beat + n · 60 / bpm` seconds. Detected tempo is never a guess: a track
/// without a clear tempo has no grid at all.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeatGrid {
    pub bpm: f64,
    /// Time of the first beat in seconds, `0 ≤ first_beat < period`.
    pub first_beat: f64,
}

impl BeatGrid {
    /// Seconds per beat.
    pub fn period(&self) -> f64 {
        60.0 / self.bpm
    }

    /// Time of beat `n` (may be negative for beats before the first).
    pub fn beat_time(&self, n: i64) -> f64 {
        self.first_beat + n as f64 * self.period()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beats_are_spaced_by_the_period() {
        let g = BeatGrid {
            bpm: 120.0,
            first_beat: 0.1,
        };
        assert_eq!(g.period(), 0.5);
        assert!((g.beat_time(4) - 2.1).abs() < 1e-12);
        assert!((g.beat_time(-1) + 0.4).abs() < 1e-12);
    }
}
