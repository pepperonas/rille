/// A fully decoded track, resampled to the engine rate. Always stereo, interleaved `L R L R …`.
///
/// Shared between threads as `Arc<TrackAudio>`; the audio thread only ever reads it and never
/// drops the last reference (see `.claude/rules/audio-realtime.md`).
#[derive(Debug, Clone, PartialEq)]
pub struct TrackAudio {
    pub id: u64,
    pub sample_rate: u32,
    pub samples: Vec<f32>,
}

impl TrackAudio {
    pub const CHANNELS: usize = 2;

    pub fn new(id: u64, sample_rate: u32, samples: Vec<f32>) -> TrackAudio {
        TrackAudio {
            id,
            sample_rate,
            samples,
        }
    }

    /// Number of stereo frames. A trailing half frame (odd sample count) is ignored.
    pub fn frames(&self) -> usize {
        self.samples.len() / Self::CHANNELS
    }

    /// Frame at `index`, or silence past the end.
    #[inline]
    pub fn frame(&self, index: usize) -> [f32; 2] {
        let i = index * Self::CHANNELS;
        match (self.samples.get(i), self.samples.get(i + 1)) {
            (Some(&l), Some(&r)) => [l, r],
            _ => [0.0, 0.0],
        }
    }

    pub fn duration_secs(&self) -> f64 {
        if self.sample_rate == 0 {
            return 0.0;
        }
        self.frames() as f64 / f64::from(self.sample_rate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_ignore_half_frame() {
        let t = TrackAudio::new(1, 48_000, vec![0.0; 5]);
        assert_eq!(t.frames(), 2);
    }

    #[test]
    fn frame_past_end_is_silence() {
        let t = TrackAudio::new(1, 48_000, vec![0.1, 0.2, 0.3, 0.4]);
        assert_eq!(t.frame(1), [0.3, 0.4]);
        assert_eq!(t.frame(2), [0.0, 0.0]);
    }

    #[test]
    fn duration_handles_zero_rate() {
        assert_eq!(TrackAudio::new(1, 0, vec![0.0; 4]).duration_secs(), 0.0);
        assert_eq!(TrackAudio::new(1, 2, vec![0.0; 8]).duration_secs(), 2.0);
    }
}
