//! Offline sample-rate conversion of a whole track (FFT resampler, high quality).

use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Resampler};

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_absurd_rates_without_allocating_for_them() {
        assert!(resample_stereo(&[0.0; 4], 4_000_000_000, 48_000).is_err());
        assert!(resample_stereo(&[0.0; 4], 44_100, 0).is_err());
    }

    #[test]
    fn same_rate_is_identity() {
        assert_eq!(resample_stereo(&[0.1, 0.2, 0.3], 48_000, 48_000).unwrap(), vec![0.1, 0.2]);
    }
}
