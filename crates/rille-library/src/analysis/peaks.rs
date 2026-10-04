//! Waveform peaks in three bands (low < 200 Hz, mid, high > 2 kHz) at several zoom levels,
//! and their binary file format. The waveforms in M6 draw from this.
//!
//! Format v1, little endian:
//! ```text
//! "RLPK" · u16 version = 1 · u8 bands = 3 · u8 levels · u32 sample_rate · u32 frames_per_bin
//! per level: u32 bins · bins × [low, mid, high] as u8
//! ```
//! Level `k` covers `frames_per_bin · 4^k` frames per bin. Values are the band's peak
//! amplitude, square-root scaled to 0..255 (quiet detail stays visible).

use std::f32::consts::PI;

pub const MAGIC: &[u8; 4] = b"RLPK";
pub const VERSION: u16 = 1;
pub const BANDS: usize = 3;
pub const LEVELS: usize = 4;
/// Zoom factor between levels.
pub const LEVEL_FACTOR: usize = 4;
const LOW_HZ: f32 = 200.0;
const HIGH_HZ: f32 = 2_000.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Peaks {
    pub sample_rate: u32,
    /// Source frames per bin at level 0.
    pub frames_per_bin: u32,
    /// `levels[k][bin] = [low, mid, high]`.
    pub levels: Vec<Vec<[u8; BANDS]>>,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum PeaksError {
    #[error("keine Peaks-Datei")]
    NotPeaks,
    #[error("Peaks-Version {0} wird nicht unterstützt")]
    Version(u16),
    #[error("Peaks-Datei ist unvollständig")]
    Truncated,
}

impl Peaks {
    pub fn to_bytes(&self) -> Vec<u8> {
        let size: usize = 16
            + self
                .levels
                .iter()
                .map(|l| 4 + l.len() * BANDS)
                .sum::<usize>();
        let mut out = Vec::with_capacity(size);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.push(BANDS as u8);
        out.push(self.levels.len() as u8);
        out.extend_from_slice(&self.sample_rate.to_le_bytes());
        out.extend_from_slice(&self.frames_per_bin.to_le_bytes());
        for level in &self.levels {
            out.extend_from_slice(&(level.len() as u32).to_le_bytes());
            out.extend(level.iter().flatten());
        }
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Peaks, PeaksError> {
        let mut r = Reader { bytes, at: 0 };
        if r.take(4)? != MAGIC {
            return Err(PeaksError::NotPeaks);
        }
        let version = u16::from_le_bytes([r.u8()?, r.u8()?]);
        if version != VERSION {
            return Err(PeaksError::Version(version));
        }
        if usize::from(r.u8()?) != BANDS {
            return Err(PeaksError::NotPeaks);
        }
        let levels = usize::from(r.u8()?);
        let sample_rate = r.u32()?;
        let frames_per_bin = r.u32()?;
        let mut out = Vec::with_capacity(levels);
        for _ in 0..levels {
            let bins = r.u32()? as usize;
            let data = r.take(bins.checked_mul(BANDS).ok_or(PeaksError::Truncated)?)?;
            out.push(data.as_chunks::<BANDS>().0.to_vec());
        }
        if r.at != bytes.len() {
            return Err(PeaksError::Truncated);
        }
        Ok(Peaks {
            sample_rate,
            frames_per_bin,
            levels: out,
        })
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], PeaksError> {
        let end = self.at.checked_add(n).ok_or(PeaksError::Truncated)?;
        let slice = self.bytes.get(self.at..end).ok_or(PeaksError::Truncated)?;
        self.at = end;
        Ok(slice)
    }
    fn u8(&mut self) -> Result<u8, PeaksError> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, PeaksError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
}

/// Second-order section (RBJ cookbook), direct form I.
#[derive(Clone, Copy)]
struct Biquad {
    b: [f32; 3],
    a: [f32; 2],
    x: [f32; 2],
    y: [f32; 2],
}

impl Biquad {
    fn new(rate: u32, hz: f32, high_pass: bool) -> Biquad {
        let w = 2.0 * PI * (hz / rate as f32).min(0.49);
        let (sin, cos) = w.sin_cos();
        let alpha = sin / (2.0 * std::f32::consts::FRAC_1_SQRT_2);
        let a0 = 1.0 + alpha;
        let b = if high_pass {
            [(1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0]
        } else {
            [(1.0 - cos) / 2.0, 1.0 - cos, (1.0 - cos) / 2.0]
        };
        Biquad {
            b: b.map(|v| v / a0),
            a: [-2.0 * cos / a0, (1.0 - alpha) / a0],
            x: [0.0; 2],
            y: [0.0; 2],
        }
    }

    fn tick(&mut self, x: f32) -> f32 {
        let y = self.b[0] * x + self.b[1] * self.x[0] + self.b[2] * self.x[1]
            - self.a[0] * self.y[0]
            - self.a[1] * self.y[1];
        self.x = [x, self.x[0]];
        self.y = [y, self.y[0]];
        y
    }
}

/// Builds [`Peaks`] from mono samples as they stream in.
pub struct PeakBuilder {
    sample_rate: u32,
    frames_per_bin: usize,
    low: [Biquad; 2],
    mid: [Biquad; 2],
    high: [Biquad; 2],
    current: [f32; BANDS],
    in_bin: usize,
    bins: Vec<[u8; BANDS]>,
}

impl PeakBuilder {
    pub fn new(sample_rate: u32) -> PeakBuilder {
        // ~5 ms per bin at level 0 for any rate.
        let frames_per_bin = 256 * (sample_rate as usize).div_ceil(48_000).max(1);
        PeakBuilder {
            sample_rate,
            frames_per_bin,
            // Two cascaded sections per edge: 24 dB/octave, so the bands really separate.
            low: [Biquad::new(sample_rate, LOW_HZ, false); 2],
            mid: [
                Biquad::new(sample_rate, LOW_HZ, true),
                Biquad::new(sample_rate, HIGH_HZ, false),
            ],
            high: [Biquad::new(sample_rate, HIGH_HZ, true); 2],
            current: [0.0; BANDS],
            in_bin: 0,
            bins: Vec::new(),
        }
    }

    pub fn push(&mut self, mono: &[f32]) {
        for &s in mono {
            let low = cascade(&mut self.low, s);
            let mid = cascade(&mut self.mid, s);
            let high = cascade(&mut self.high, s);
            for (c, v) in self.current.iter_mut().zip([low, mid, high]) {
                *c = c.max(v.abs());
            }
            self.in_bin += 1;
            if self.in_bin == self.frames_per_bin {
                self.close_bin();
            }
        }
    }

    fn close_bin(&mut self) {
        self.bins.push(self.current.map(to_u8));
        self.current = [0.0; BANDS];
        self.in_bin = 0;
    }

    pub fn finish(mut self) -> Peaks {
        if self.in_bin > 0 {
            self.close_bin();
        }
        let mut levels = vec![self.bins];
        while levels.len() < LEVELS {
            let Some(prev) = levels.last() else { break };
            let next = prev
                .chunks(LEVEL_FACTOR)
                .map(|group| {
                    group.iter().fold([0u8; BANDS], |acc, b| {
                        [acc[0].max(b[0]), acc[1].max(b[1]), acc[2].max(b[2])]
                    })
                })
                .collect();
            levels.push(next);
        }
        Peaks {
            sample_rate: self.sample_rate,
            frames_per_bin: self.frames_per_bin as u32,
            levels,
        }
    }
}

fn cascade(stages: &mut [Biquad; 2], x: f32) -> f32 {
    let y = stages[0].tick(x);
    stages[1].tick(y)
}

fn to_u8(peak: f32) -> u8 {
    (peak.clamp(0.0, 1.0).sqrt() * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(rate: u32, hz: f32, seconds: f32) -> Vec<f32> {
        (0..(rate as f32 * seconds) as usize)
            .map(|i| 0.5 * (i as f32 * hz * 2.0 * PI / rate as f32).sin())
            .collect()
    }

    fn peaks_of(samples: &[f32]) -> Peaks {
        let mut b = PeakBuilder::new(44_100);
        for chunk in samples.chunks(777) {
            b.push(chunk);
        }
        b.finish()
    }

    #[test]
    fn bands_separate_low_mid_and_high() {
        // Skip the first bins: filters settle.
        let body =
            |p: &Peaks, band: usize| p.levels[0][20..].iter().map(|b| b[band]).max().unwrap_or(0);
        let low = peaks_of(&tone(44_100, 60.0, 1.0));
        let mid = peaks_of(&tone(44_100, 630.0, 1.0));
        let high = peaks_of(&tone(44_100, 6_300.0, 1.0));
        assert!(
            body(&low, 0) > 150 && body(&low, 2) < 40,
            "low {:?}",
            low.levels[0][30]
        );
        assert!(
            body(&mid, 1) > 150 && body(&mid, 0) < 120,
            "mid {:?}",
            mid.levels[0][30]
        );
        assert!(
            body(&high, 2) > 150 && body(&high, 0) < 40,
            "high {:?}",
            high.levels[0][30]
        );
    }

    #[test]
    fn levels_are_maxima_of_four() {
        let mut samples = vec![0.0f32; 44_100];
        samples[30_000] = 0.9; // one spike
        let p = peaks_of(&samples);
        assert_eq!(p.levels.len(), LEVELS);
        for k in 1..LEVELS {
            assert_eq!(
                p.levels[k].len(),
                p.levels[k - 1].len().div_ceil(LEVEL_FACTOR)
            );
            let max = |l: &Vec<[u8; 3]>| l.iter().map(|b| b[2]).max();
            assert_eq!(max(&p.levels[k]), max(&p.levels[k - 1]), "level {k}");
        }
        // The spike is in the right bin at level 0.
        let bin = 30_000 / p.frames_per_bin as usize;
        assert!(p.levels[0][bin][2] > 100);
    }

    #[test]
    fn roundtrip_through_bytes() {
        let p = peaks_of(&tone(44_100, 440.0, 0.3));
        assert_eq!(Peaks::from_bytes(&p.to_bytes()), Ok(p));
    }

    #[test]
    fn bad_files_are_rejected() {
        let bytes = peaks_of(&tone(44_100, 440.0, 0.3)).to_bytes();
        assert_eq!(
            Peaks::from_bytes(&bytes[..bytes.len() - 1]),
            Err(PeaksError::Truncated)
        );
        let mut longer = bytes.clone();
        longer.push(0);
        assert_eq!(Peaks::from_bytes(&longer), Err(PeaksError::Truncated));
        assert_eq!(Peaks::from_bytes(b"nope"), Err(PeaksError::NotPeaks));
        let mut future = bytes;
        future[4] = 9;
        assert_eq!(Peaks::from_bytes(&future), Err(PeaksError::Version(9)));
        // A bin count that overflows must not panic or allocate.
        let mut huge = b"RLPK\x01\x00\x03\x01".to_vec();
        huge.extend_from_slice(&44_100u32.to_le_bytes());
        huge.extend_from_slice(&256u32.to_le_bytes());
        huge.extend_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(Peaks::from_bytes(&huge), Err(PeaksError::Truncated));
    }

    #[test]
    fn empty_input_gives_empty_levels() {
        let p = PeakBuilder::new(48_000).finish();
        assert!(p.levels.iter().all(Vec::is_empty));
        assert_eq!(Peaks::from_bytes(&p.to_bytes()), Ok(p));
    }
}
