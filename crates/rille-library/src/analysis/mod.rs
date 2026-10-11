//! Track analysis: onset envelope → tempo and beatgrid; three-band waveform peaks.
//!
//! [`analyze_file`] streams the file once through both, without holding the decoded audio:
//! memory stays flat even for hour-long mixes on several workers.

pub mod bpm;
pub mod onset;
pub mod peaks;
#[cfg(test)]
pub(crate) mod synth;

use std::path::Path;

use rille_core::BeatGrid;

use crate::decode::{DecodeError, read_blocks};
use onset::OnsetDetector;
use peaks::{PeakBuilder, Peaks};

#[derive(Debug, Clone, PartialEq)]
pub struct Analysis {
    /// `None` when the track has no clear tempo.
    pub grid: Option<BeatGrid>,
    /// The reading behind `grid`, also when it fell below the confidence threshold
    /// (diagnostics).
    pub reading: Option<bpm::Reading>,
    pub peaks: Peaks,
    pub duration_secs: f64,
}

/// Decode `path` once and analyse it. Stops early (with an error) if `keep_going` turns false.
pub fn analyze_file(
    path: &Path,
    mut keep_going: impl FnMut() -> bool,
) -> Result<Analysis, DecodeError> {
    let mut state: Option<(u32, OnsetDetector, PeakBuilder)> = None;
    let mut mono = Vec::new();
    let mut cancelled = false;
    let frames = read_blocks(
        path,
        || {
            let go = keep_going();
            cancelled |= !go;
            go
        },
        |rate, stereo| {
            let (_, onset, peaks) = state
                .get_or_insert_with(|| (rate, OnsetDetector::new(rate), PeakBuilder::new(rate)));
            to_mono(stereo, &mut mono);
            onset.push(&mono);
            peaks.push(&mono);
        },
    )?;
    if cancelled {
        return Err(DecodeError::Cancelled);
    }
    let (rate, onset, peaks) = state.ok_or(DecodeError::Empty)?;
    let envelope = onset.finish();
    Ok(Analysis {
        grid: bpm::estimate(&envelope),
        reading: bpm::reading(&envelope),
        peaks: peaks.finish(),
        duration_secs: frames as f64 / f64::from(rate),
    })
}

/// Interleaved stereo to mono. Non-finite samples (a damaged float file) become silence: a
/// single NaN would otherwise poison the filters for the rest of the track.
fn to_mono(stereo: &[f32], mono: &mut Vec<f32>) {
    mono.clear();
    mono.extend(stereo.as_chunks::<2>().0.iter().map(|f| {
        let m = 0.5 * (f[0] + f[1]);
        if m.is_finite() { m } else { 0.0 }
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broken_samples_do_not_poison_the_peaks() {
        let mut stereo: Vec<f32> = (0..48_000)
            .flat_map(|i| {
                let v = 0.5 * (i as f32 * 0.3).sin();
                [v, v]
            })
            .collect();
        stereo[1_000] = f32::NAN;
        stereo[2_001] = f32::INFINITY;
        let mut mono = Vec::new();
        to_mono(&stereo, &mut mono);
        assert!(mono.iter().all(|v| v.is_finite()));
        let mut peaks = PeakBuilder::new(48_000);
        peaks.push(&mono);
        let p = peaks.finish();
        let last = p.levels[0]
            .last()
            .map_or(0, |b| b.iter().copied().max().unwrap_or(0));
        assert!(last > 100, "the end still has a waveform: {last}");
    }
}
