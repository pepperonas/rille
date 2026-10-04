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
            mono.clear();
            mono.extend(
                stereo
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|f| 0.5 * (f[0] + f[1])),
            );
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
