//! Offline sample-rate conversion of a whole track (FFT resampler, high quality).

use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Indexing, Resampler};

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct ResampleError(String);

/// Resample interleaved stereo from `from` Hz to `to` Hz. Output length is
/// `ceil(frames · to / from)` frames.
/// Rates must lie in `MIN_RATE..=MAX_RATE`; anything else is rejected before allocating.
pub fn resample_stereo(input: &[f32], from: u32, to: u32) -> Result<Vec<f32>, ResampleError> {
    use crate::decode::{MAX_RATE, MIN_RATE};
    for rate in [from, to] {
        if !(MIN_RATE..=MAX_RATE).contains(&rate) {
            return Err(ResampleError(format!("sample rate {rate} Hz out of range")));
        }
    }
    let frames = input.len() / 2;
    if frames == 0 || from == to {
        return Ok(input[..frames * 2].to_vec());
    }
    let err = |e: &dyn std::fmt::Display| ResampleError(e.to_string());
    let mut resampler = Fft::<f32>::new(from as usize, to as usize, 4096, 2, FixedSync::Input)
        .map_err(|e| err(&e))?;
    let adapter = InterleavedSlice::new(&input[..frames * 2], 2, frames).map_err(|e| err(&e))?;
    let out = resampler
        .process_all(&adapter, frames, None)
        .map_err(|e| err(&e))?;
    Ok(out.take_data())
}

/// Sample-rate conversion in pieces, so a track can be played while it is still decoding.
/// Same resampler, chunk size and delay trimming as [`resample_stereo`], so the result is the
/// same as converting the whole track at once (tested).
pub struct StreamResampler {
    inner: Fft<f32>,
    /// Interleaved stereo input not yet consumed.
    pending: Vec<f32>,
    out: Vec<f32>,
    /// Output frames still to drop: the resampler's startup delay.
    to_trim: usize,
    total_in: usize,
    total_out: usize,
    from: u32,
    to: u32,
}

/// `ceil(frames · to / from)` in integers: the float version lands one frame high for exact
/// ratios (88 200 · 48 000 / 44 100 = 96 000.000…01).
pub fn converted_len(frames: usize, from: u32, to: u32) -> usize {
    let (frames, from, to) = (frames as u128, u128::from(from.max(1)), u128::from(to));
    ((frames * to).div_ceil(from)) as usize
}

impl StreamResampler {
    pub fn new(from: u32, to: u32) -> Result<StreamResampler, ResampleError> {
        use crate::decode::{MAX_RATE, MIN_RATE};
        for rate in [from, to] {
            if !(MIN_RATE..=MAX_RATE).contains(&rate) {
                return Err(ResampleError(format!("sample rate {rate} Hz out of range")));
            }
        }
        let inner = Fft::<f32>::new(from as usize, to as usize, 4096, 2, FixedSync::Input)
            .map_err(|e| ResampleError(e.to_string()))?;
        Ok(StreamResampler {
            to_trim: inner.output_delay(),
            out: vec![0.0; inner.output_frames_max() * 2],
            inner,
            pending: Vec::new(),
            total_in: 0,
            total_out: 0,
            from,
            to,
        })
    }

    /// Feed interleaved stereo; `emit` receives converted samples as they become available.
    pub fn push(
        &mut self,
        stereo: &[f32],
        emit: &mut dyn FnMut(&[f32]),
    ) -> Result<(), ResampleError> {
        let frames = stereo.len() / 2;
        self.pending.extend_from_slice(&stereo[..frames * 2]);
        self.total_in += frames;
        let mut consumed = 0;
        while self.pending.len() / 2 - consumed >= self.inner.input_frames_next() {
            consumed += self.run(consumed, None, usize::MAX, emit)?;
        }
        self.pending.drain(..consumed * 2);
        Ok(())
    }

    /// End of input: convert the rest and pad out to exactly `ceil(frames · to / from)`.
    pub fn finish(mut self, emit: &mut dyn FnMut(&[f32])) -> Result<(), ResampleError> {
        let expected = converted_len(self.total_in, self.from, self.to);
        let rest = self.pending.len() / 2;
        if rest > 0 {
            self.run(0, Some(rest), expected, emit)?;
        }
        // Zeros push the remaining delayed output through.
        while self.total_out < expected {
            self.run(0, Some(0), expected, emit)?;
        }
        Ok(())
    }

    /// One resampler call on `pending` from frame `offset`. `partial`: `None` for a full
    /// chunk, `Some(n)` for the last `n` frames (`0` = only zeros). Emits at most up to `cap`
    /// output frames in total. Returns the input frames used.
    fn run(
        &mut self,
        offset: usize,
        partial: Option<usize>,
        cap: usize,
        emit: &mut dyn FnMut(&[f32]),
    ) -> Result<usize, ResampleError> {
        let err = |e: &dyn std::fmt::Display| ResampleError(e.to_string());
        // An empty buffer is not a valid adapter; one silent frame stands in (it is not read
        // when `partial` is 0).
        if self.pending.is_empty() {
            self.pending.extend_from_slice(&[0.0, 0.0]);
        }
        let input = InterleavedSlice::new(&self.pending[..], 2, self.pending.len() / 2)
            .map_err(|e| err(&e))?;
        let max_out = self.out.len() / 2;
        let mut output =
            InterleavedSlice::new_mut(&mut self.out[..], 2, max_out).map_err(|e| err(&e))?;
        let indexing = Indexing {
            input_offset: offset,
            output_offset: 0,
            active_channels_mask: None,
            partial_len: partial,
        };
        let (used, produced) = self
            .inner
            .process_into_buffer(&input, &mut output, Some(&indexing))
            .map_err(|e| err(&e))?;
        self.emit_trimmed(produced, cap, emit);
        Ok(used)
    }

    /// Drop the startup delay, then pass on what is left, never beyond `cap` frames in total.
    fn emit_trimmed(&mut self, produced: usize, cap: usize, emit: &mut dyn FnMut(&[f32])) {
        let skip = self.to_trim.min(produced);
        self.to_trim -= skip;
        let keep = (produced - skip).min(cap.saturating_sub(self.total_out));
        if keep > 0 {
            emit(&self.out[skip * 2..(skip + keep) * 2]);
            self.total_out += keep;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_absurd_rates_without_allocating_for_them() {
        assert!(resample_stereo(&[0.0; 4], 4_000_000_000, 48_000).is_err());
        assert!(resample_stereo(&[0.0; 4], 44_100, 0).is_err());
    }

    #[test]
    fn streaming_matches_converting_at_once() {
        let n = 50_000;
        let input: Vec<f32> = (0..n)
            .flat_map(|i| {
                let t = i as f32 / 44_100.0;
                [
                    (t * 440.0 * std::f32::consts::TAU).sin(),
                    (t * 97.0 * std::f32::consts::TAU).sin() * 0.5,
                ]
            })
            .collect();
        let whole = resample_stereo(&input, 44_100, 48_000).unwrap();
        let mut s = StreamResampler::new(44_100, 48_000).unwrap();
        let mut streamed = Vec::new();
        // Irregular piece sizes, like decoder packets.
        let mut at = 0;
        for size in [1152usize, 7, 4096, 333, 9000].iter().cycle() {
            if at >= n {
                break;
            }
            let end = (at + size).min(n);
            s.push(&input[at * 2..end * 2], &mut |o| {
                streamed.extend_from_slice(o)
            })
            .unwrap();
            at = end;
        }
        s.finish(&mut |o| streamed.extend_from_slice(o)).unwrap();
        assert_eq!(streamed.len(), whole.len());
        let worst = streamed
            .iter()
            .zip(&whole)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        assert!(worst < 1e-5, "differs by {worst}");
    }

    #[test]
    fn short_clip_streams_too() {
        let input = vec![0.25f32; 2 * 100];
        let whole = resample_stereo(&input, 44_100, 48_000).unwrap();
        let mut s = StreamResampler::new(44_100, 48_000).unwrap();
        let mut streamed = Vec::new();
        s.push(&input, &mut |o| streamed.extend_from_slice(o))
            .unwrap();
        s.finish(&mut |o| streamed.extend_from_slice(o)).unwrap();
        assert_eq!(streamed.len(), whole.len());
    }

    #[test]
    fn same_rate_is_identity() {
        assert_eq!(
            resample_stereo(&[0.1, 0.2, 0.3], 48_000, 48_000).unwrap(),
            vec![0.1, 0.2]
        );
    }
}
