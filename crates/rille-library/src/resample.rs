//! Offline sample-rate conversion of a whole track (FFT resampler, high quality).

use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Resampler};

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct ResampleError(String);

/// Resample interleaved stereo from `from` Hz to `to` Hz. Output length is
/// `ceil(frames · to / from)` frames.
pub fn resample_stereo(input: &[f32], from: u32, to: u32) -> Result<Vec<f32>, ResampleError> {
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
