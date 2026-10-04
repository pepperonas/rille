//! Decoded track audio, readable by the audio thread while it is still being decoded.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

/// A track resampled to the engine rate. Always stereo, interleaved `L R L R …`.
///
/// **Progressive:** a track can be handed to the engine before decoding has finished. The
/// loader (the only writer) fills the preallocated buffer front to back and publishes how many
/// frames are ready; the audio thread reads only published frames and hears silence beyond.
/// Samples are stored as `f32` bits in `AtomicU32`, published with Release/Acquire on the
/// `ready` counter, so reader and writer never need a lock. Relaxed atomic loads compile to
/// plain loads, the audio thread pays nothing for this.
///
/// [`TrackAudio::frames`] is the expected length while loading (exact for most formats,
/// estimated for MP3) and the real length once [`TrackAudio::finish`] has run.
///
/// Shared between threads as `Arc<TrackAudio>`; the audio thread only ever reads it and never
/// drops the last reference (see `.claude/rules/audio-realtime.md`).
#[derive(Debug)]
pub struct TrackAudio {
    pub id: u64,
    pub sample_rate: u32,
    samples: Box<[AtomicU32]>,
    /// Frames the writer has published.
    ready: AtomicUsize,
    /// Track length in frames (expected until `complete`).
    len: AtomicUsize,
    complete: AtomicBool,
}

/// Zero-initialised atomics without touching every page: `vec![0u32; n]` asks the allocator
/// for zeroed memory, which the OS maps lazily. Building the atomics one by one would write
/// every page up front (hundreds of MB for a long track).
fn zeroed_atomics(samples: usize) -> Box<[AtomicU32]> {
    let zeroed: Box<[u32]> = vec![0u32; samples].into_boxed_slice();
    let raw = Box::into_raw(zeroed) as *mut [AtomicU32];
    // SAFETY: `AtomicU32` has the same size, alignment and bit validity as `u32` (documented
    // guarantee of `core::sync::atomic`), so the allocation is a valid `[AtomicU32]` of the
    // same length and layout, and ownership moves into the new box exactly once.
    unsafe { Box::from_raw(raw) }
}

impl TrackAudio {
    pub const CHANNELS: usize = 2;

    /// A complete track from interleaved stereo samples. A trailing half frame is ignored.
    pub fn new(id: u64, sample_rate: u32, samples: Vec<f32>) -> TrackAudio {
        let frames = samples.len() / Self::CHANNELS;
        let track = TrackAudio::streaming(id, sample_rate, frames, frames);
        track.write(0, &samples[..frames * Self::CHANNELS]);
        track.finish(frames);
        track
    }

    /// An empty track to be filled while it plays. `expected` frames is the length shown until
    /// [`TrackAudio::finish`]; `capacity` (≥ `expected`) is the most that can be written.
    pub fn streaming(id: u64, sample_rate: u32, expected: usize, capacity: usize) -> TrackAudio {
        let capacity = capacity.max(expected);
        TrackAudio {
            id,
            sample_rate,
            samples: zeroed_atomics(capacity * Self::CHANNELS),
            ready: AtomicUsize::new(0),
            len: AtomicUsize::new(expected),
            complete: AtomicBool::new(false),
        }
    }

    /// Track length in frames: expected while loading, exact once complete.
    pub fn frames(&self) -> usize {
        self.len.load(Ordering::Acquire)
    }

    /// Frames that can be read right now.
    pub fn ready_frames(&self) -> usize {
        self.ready.load(Ordering::Acquire)
    }

    pub fn capacity(&self) -> usize {
        self.samples.len() / Self::CHANNELS
    }

    pub fn is_complete(&self) -> bool {
        self.complete.load(Ordering::Acquire)
    }

    /// Frame at `index`, or silence where nothing is published (yet) or past the end.
    #[inline]
    pub fn frame(&self, index: usize) -> [f32; 2] {
        if index >= self.ready.load(Ordering::Acquire) {
            return [0.0, 0.0];
        }
        let i = index * Self::CHANNELS;
        match (self.samples.get(i), self.samples.get(i + 1)) {
            (Some(l), Some(r)) => [
                f32::from_bits(l.load(Ordering::Relaxed)),
                f32::from_bits(r.load(Ordering::Relaxed)),
            ],
            _ => [0.0, 0.0],
        }
    }

    /// Copy of the published samples (tests, analysis). Not for the audio thread.
    pub fn to_vec(&self) -> Vec<f32> {
        let n = self.ready_frames().min(self.frames()) * Self::CHANNELS;
        self.samples[..n]
            .iter()
            .map(|s| f32::from_bits(s.load(Ordering::Relaxed)))
            .collect()
    }

    /// Write interleaved stereo samples starting at frame `at` and publish everything up to
    /// their end. Writer only (one thread); frames must be written front to back. Samples past
    /// the capacity are dropped. Returns the frames written.
    pub fn write(&self, at: usize, stereo: &[f32]) -> usize {
        let start = (at * Self::CHANNELS).min(self.samples.len());
        let end = (start + stereo.len() / Self::CHANNELS * Self::CHANNELS).min(self.samples.len());
        for (slot, v) in self.samples[start..end].iter().zip(stereo) {
            slot.store(v.to_bits(), Ordering::Relaxed);
        }
        let written = (end - start) / Self::CHANNELS;
        // Release: a reader that sees the new `ready` also sees the samples stored above.
        self.ready
            .fetch_max(start / Self::CHANNELS + written, Ordering::Release);
        written
    }

    /// Decoding is done after `frames` frames (clamped to what was published).
    pub fn finish(&self, frames: usize) {
        let frames = frames.min(self.ready_frames());
        self.len.store(frames, Ordering::Release);
        self.complete.store(true, Ordering::Release);
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
    use std::sync::Arc;

    #[test]
    fn frames_ignore_half_frame() {
        let t = TrackAudio::new(1, 48_000, vec![0.0; 5]);
        assert_eq!(t.frames(), 2);
        assert!(t.is_complete());
    }

    #[test]
    fn frame_past_end_is_silence() {
        let t = TrackAudio::new(1, 48_000, vec![0.1, 0.2, 0.3, 0.4]);
        assert_eq!(t.frame(1), [0.3, 0.4]);
        assert_eq!(t.frame(2), [0.0, 0.0]);
        assert_eq!(t.to_vec(), [0.1, 0.2, 0.3, 0.4]);
    }

    #[test]
    fn duration_handles_zero_rate() {
        assert_eq!(TrackAudio::new(1, 0, vec![0.0; 4]).duration_secs(), 0.0);
        assert_eq!(TrackAudio::new(1, 2, vec![0.0; 8]).duration_secs(), 2.0);
    }

    #[test]
    fn unpublished_frames_read_as_silence() {
        let t = TrackAudio::streaming(1, 48_000, 4, 6);
        assert_eq!(t.frames(), 4, "expected length while loading");
        assert_eq!(t.frame(0), [0.0, 0.0]);
        assert_eq!(t.write(0, &[0.5, 0.5, 0.25, 0.25]), 2);
        assert_eq!(t.ready_frames(), 2);
        assert_eq!(t.frame(1), [0.25, 0.25]);
        assert_eq!(t.frame(2), [0.0, 0.0]);
        assert!(!t.is_complete());
    }

    #[test]
    fn finish_corrects_the_length_both_ways() {
        // An MP3 estimate can be too long …
        let t = TrackAudio::streaming(1, 48_000, 10, 12);
        t.write(0, &[0.1; 2 * 7]);
        t.finish(7);
        assert_eq!((t.frames(), t.is_complete()), (7, true));
        // … or too short; the capacity margin takes the rest.
        let t = TrackAudio::streaming(1, 48_000, 10, 12);
        t.write(0, &[0.1; 2 * 12]);
        t.finish(12);
        assert_eq!(t.frames(), 12);
        // Never longer than what was published.
        let t = TrackAudio::streaming(1, 48_000, 10, 12);
        t.write(0, &[0.1; 2 * 3]);
        t.finish(10);
        assert_eq!(t.frames(), 3);
    }

    #[test]
    fn writes_beyond_capacity_are_dropped() {
        let t = TrackAudio::streaming(1, 48_000, 2, 2);
        assert_eq!(t.write(1, &[0.1; 2 * 5]), 1);
        assert_eq!(t.ready_frames(), 2);
        assert_eq!(t.write(5, &[0.1; 2]), 0);
    }

    #[test]
    fn a_reader_sees_every_published_sample_while_the_writer_runs() {
        let frames = 200_000;
        let t = Arc::new(TrackAudio::streaming(1, 48_000, frames, frames));
        let writer = {
            let t = t.clone();
            std::thread::spawn(move || {
                let mut at = 0;
                while at < frames {
                    let n = 997.min(frames - at);
                    let chunk: Vec<f32> = (at..at + n).flat_map(|i| [i as f32; 2]).collect();
                    at += t.write(at, &chunk);
                }
                t.finish(frames);
            })
        };
        // Whatever is published must already hold its final value.
        while !t.is_complete() {
            let ready = t.ready_frames();
            if ready > 0 {
                let i = ready - 1;
                assert_eq!(t.frame(i), [i as f32; 2]);
            }
        }
        let _ = writer.join();
        assert_eq!(t.frame(frames - 1), [(frames - 1) as f32; 2]);
    }
}
