//! Tempo and beatgrid from an onset envelope.
//!
//! Measured on real tracks (`examples/analyze`), not only synthetic ones: the full band gives
//! a robust tempo; the low band (< 200 Hz) does not, because bass lines put their own onsets
//! there. But the low band knows where the kicks are, which is where DJs put the beat — so
//! tempo comes from the full band and the low band only picks the phase.
//!
//! 1. **Coarse tempo** from the autocorrelation of the full-band envelope over 60–200 BPM, then
//!    the fastest of its octaves that is nearly as well supported (kicks on every beat at 174
//!    support 174 as well as 87, so 174; kicks at 90 with nothing between leave 180
//!    unsupported, so 90).
//! 2. **DJ range:** the tempo is folded into 70–180 BPM by doubling or halving (Bicep "Glue"
//!    reads 65 otherwise, not 130). Ambient tracks below 70 therefore show twice their tempo,
//!    as in other DJ software.
//! 3. **Fine tempo** by folding: the envelope is wrapped onto one beat period; at the right
//!    tempo all beats land in one sharp peak, a tempo slightly off smears them across the
//!    track. The sharpest fold within ±2 % gives the BPM to 0.005.
//! 4. **Phase:** of the strongest peaks in that fold (kick, snare, hi-hat), the one with the
//!    most low-band energy is the beat; otherwise hi-hats can put the grid on the offbeat.
//! 5. **No guessing:** too few beats, silence or a fold without a clear peak give `None`.
//!
//! Known limit: half-time drum'n'bass (kick on 1, snare on 3) reads as ~87 rather than 174.

use rille_core::BeatGrid;

use super::onset::Envelope;

pub const MIN_BPM: f64 = 60.0;
pub const MAX_BPM: f64 = 200.0;
/// Tempos are reported in this range (by doubling or halving).
pub const RANGE_MIN: f64 = 70.0;
pub const RANGE_MAX: f64 = 180.0;
/// Phase bins per beat when folding.
const FOLD_BINS: usize = 96;
/// A fold must stand out this much (peak over mean) to count as a tempo. Measured: noise folds
/// to ~2.5, real tracks to 3.5–20 (Fleetwood Mac "Dreams", live drums: 3.5), synthetic beats
/// to 25–50.
const MIN_SHARPNESS: f64 = 3.0;
/// Fewer beats than this is not a rhythm.
const MIN_BEATS: f64 = 4.0;
/// Window of the moving average removed from the envelope (seconds).
const DETREND_SECONDS: f64 = 0.4;
/// A faster octave is taken if its support is at least this share of the best.
const OCTAVE_SUPPORT: f64 = 0.8;
/// Coarse tempo grid.
const COARSE_STEP: f64 = 0.25;
/// Fold peaks considered as the beat.
const PHASE_CANDIDATES: usize = 3;
/// Beats per segment for the consistency check.
const SEGMENT_BEATS: f64 = 16.0;
/// Median similarity of a segment's fold to the rest of the track that counts as a steady
/// beat. Measured: steady beats 0.64–1.0 on real tracks, random hits 0.1–0.3.
const MIN_CONSISTENCY: f64 = 0.6;
/// Phase drift tolerated between segments (bins of `FOLD_BINS`, here ±⅛ beat).
const DRIFT_BINS: isize = (FOLD_BINS / 8) as isize;

/// A tempo reading before the confidence threshold (also for diagnostics).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reading {
    pub grid: BeatGrid,
    /// Tempo from the autocorrelation, before the range fold and refinement.
    pub coarse: f64,
    /// Fold peak over mean; ≥ `MIN_SHARPNESS` counts as a tempo.
    pub sharpness: f64,
    /// Median correlation of the folds of 16-beat segments with the whole track's fold
    /// (1 when the track is too short to tell).
    pub consistency: f64,
}

impl Reading {
    /// Confident enough to show: a sharp fold, consistent across the track.
    pub fn is_tempo(&self) -> bool {
        self.sharpness >= MIN_SHARPNESS && self.consistency >= MIN_CONSISTENCY
    }
}

/// Estimate the beatgrid, or `None` if there is no clear tempo.
pub fn estimate(env: &Envelope) -> Option<BeatGrid> {
    reading(env).filter(Reading::is_tempo).map(|r| r.grid)
}

pub fn reading(env: &Envelope) -> Option<Reading> {
    let window = (DETREND_SECONDS * env.rate) as usize;
    let full = detrend(&env.full, window);
    let duration = full.len() as f64 / env.rate;
    if duration * MIN_BPM / 60.0 < MIN_BEATS || full.iter().all(|v| *v == 0.0) {
        return None;
    }
    let low = detrend(&env.low, window);
    let coarse = coarse_bpm(&full, &low, env.rate)?;
    let (bpm, sharpness) = refine(&full, env.rate, into_range(coarse));
    if duration * bpm / 60.0 < MIN_BEATS {
        return None;
    }
    let phase = beat_phase(&full, &low, env.rate, bpm);
    let consistency = consistency(&full, env.rate, bpm);
    let period = 60.0 / bpm;
    let first_beat = (env.offset + phase * period).rem_euclid(period);
    Some(Reading {
        grid: BeatGrid { bpm, first_beat },
        coarse,
        sharpness,
        consistency,
    })
}

/// How much each 16-beat segment's fold looks like the whole track's (median correlation). A real tempo repeats its pattern — kick, snare, hats in the same places — so
/// every segment folds to the same shape; onsets at random fold into a different chance shape
/// in every segment. Silent segments (breakdowns) do not count.
fn consistency(x: &[f32], rate: f64, bpm: f64) -> f64 {
    let per_segment = (SEGMENT_BEATS * 60.0 / bpm * rate) as usize;
    let count = x.len() / per_segment.max(1);
    if count < 3 {
        return 1.0; // too short to tell
    }
    // Folded at true positions (not from the segment start), so phases stay comparable.
    let period = rate * 60.0 / bpm;
    let segments: Vec<[f64; FOLD_BINS]> = (0..count)
        .map(|k| {
            let mut bins = [0.0f64; FOLD_BINS];
            for (i, v) in x.iter().enumerate().skip(k * per_segment).take(per_segment) {
                let pos = (i as f64 / period).fract() * FOLD_BINS as f64;
                bins[pos as usize % FOLD_BINS] += f64::from(*v);
            }
            blur(&bins)
        })
        .collect();
    let total: [f64; FOLD_BINS] =
        std::array::from_fn(|b| segments.iter().map(|s| s[b]).sum::<f64>());
    // Each segment against all the others — not against a whole that contains itself, which
    // would correlate by construction (√(1/3) with three segments).
    let mut similarities: Vec<f64> = segments
        .iter()
        .filter_map(|seg| {
            let others: [f64; FOLD_BINS] = std::array::from_fn(|b| total[b] - seg[b]);
            // Live drummers drift: the pattern may sit a little earlier or later in another
            // part of the track and is still the same pattern.
            (-DRIFT_BINS..=DRIFT_BINS)
                .filter_map(|shift| {
                    let shifted: [f64; FOLD_BINS] = std::array::from_fn(|b| {
                        seg[(b as isize + shift).rem_euclid(FOLD_BINS as isize) as usize]
                    });
                    correlation(&shifted, &others)
                })
                .reduce(f64::max)
        })
        .collect();
    if similarities.len() < 3 {
        return 1.0;
    }
    similarities.sort_by(f64::total_cmp);
    similarities[similarities.len() / 2]
}

/// Pearson correlation of two folds: their shapes, not their common baseline (any two
/// non-negative folds look alike by plain cosine). `None` for a flat (silent) fold.
fn correlation(a: &[f64; FOLD_BINS], b: &[f64; FOLD_BINS]) -> Option<f64> {
    let mean = |v: &[f64; FOLD_BINS]| v.iter().sum::<f64>() / FOLD_BINS as f64;
    let (ma, mb) = (mean(a), mean(b));
    let (mut ab, mut aa, mut bb) = (0.0, 0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        let (dx, dy) = (x - ma, y - mb);
        ab += dx * dy;
        aa += dx * dx;
        bb += dy * dy;
    }
    (aa > 0.0 && bb > 0.0).then(|| ab / (aa * bb).sqrt())
}

/// Circular smoothing over ±1 bin, so a beat a hair early or late still counts as in place.
fn blur(bins: &[f64; FOLD_BINS]) -> [f64; FOLD_BINS] {
    std::array::from_fn(|i| {
        0.25 * bins[(i + FOLD_BINS - 1) % FOLD_BINS]
            + 0.5 * bins[i]
            + 0.25 * bins[(i + 1) % FOLD_BINS]
    })
}

/// Fold a tempo into `RANGE_MIN..RANGE_MAX` by octaves.
pub fn into_range(mut bpm: f64) -> f64 {
    if bpm <= 0.0 || !bpm.is_finite() {
        return bpm;
    }
    while bpm < RANGE_MIN {
        bpm *= 2.0;
    }
    while bpm >= RANGE_MAX {
        bpm /= 2.0;
    }
    bpm
}

/// Subtract a centred moving average and keep what sticks out above it.
fn detrend(values: &[f32], window: usize) -> Vec<f32> {
    let half = (window / 2).max(1);
    let mut prefix = Vec::with_capacity(values.len() + 1);
    prefix.push(0.0f64);
    for v in values {
        let last = prefix.last().copied().unwrap_or(0.0);
        prefix.push(last + f64::from(*v));
    }
    (0..values.len())
        .map(|i| {
            let lo = i.saturating_sub(half);
            let hi = (i + half + 1).min(values.len());
            let mean = (prefix[hi] - prefix[lo]) / (hi - lo) as f64;
            (f64::from(values[i]) - mean).max(0.0) as f32
        })
        .collect()
}

/// Biased autocorrelation at a fractional lag (linear interpolation).
fn autocorr(x: &[f32], lag: f64) -> f64 {
    let n = x.len();
    let l0 = lag.floor() as usize;
    let frac = lag - l0 as f64;
    let at = |l: usize| -> f64 {
        if l >= n {
            return 0.0;
        }
        x[..n - l]
            .iter()
            .zip(&x[l..])
            .map(|(a, b)| f64::from(*a) * f64::from(*b))
            .sum::<f64>()
            / n as f64
    };
    at(l0) * (1.0 - frac) + at(l0 + 1) * frac
}

/// Support for a tempo: the best autocorrelation within ±1 % of it.
fn support(x: &[f32], rate: f64, bpm: f64) -> f64 {
    let mut best = 0.0f64;
    let mut b = bpm * 0.99;
    while b <= bpm * 1.01 {
        best = best.max(autocorr(x, rate * 60.0 / b));
        b += COARSE_STEP;
    }
    best
}

/// Best tempo by autocorrelation of the full band, then the fastest octave of it that is
/// nearly as well supported in the full band or in the low band (kicks on every beat).
fn coarse_bpm(x: &[f32], low: &[f32], rate: f64) -> Option<f64> {
    let mut best: Option<(f64, f64)> = None;
    let mut bpm = MIN_BPM;
    while bpm <= MAX_BPM {
        let score = autocorr(x, rate * 60.0 / bpm);
        if best.is_none_or(|(_, s)| score > s) {
            best = Some((bpm, score));
        }
        bpm += COARSE_STEP;
    }
    let (best_bpm, best_score) = best.filter(|(_, s)| *s > 0.0)?;
    let reference = support(x, rate, best_bpm).max(best_score);
    let low_reference = support(low, rate, best_bpm);
    let supported = |b: f64| {
        support(x, rate, b) >= OCTAVE_SUPPORT * reference
            || (low_reference > 0.0 && support(low, rate, b) >= OCTAVE_SUPPORT * low_reference)
    };
    let faster = [4.0, 2.0]
        .into_iter()
        .map(|m| best_bpm * m)
        .filter(|b| *b <= MAX_BPM * 1.01)
        .find(|b| supported(*b));
    Some(faster.unwrap_or(best_bpm).min(MAX_BPM))
}

/// The envelope wrapped onto one beat of `bpm`, as `FOLD_BINS` phase bins.
fn fold(x: &[f32], rate: f64, bpm: f64) -> [f64; FOLD_BINS] {
    let period = rate * 60.0 / bpm; // envelope values per beat
    let mut bins = [0.0f64; FOLD_BINS];
    for (i, v) in x.iter().enumerate() {
        if *v == 0.0 {
            continue;
        }
        let pos = (i as f64 / period).fract() * FOLD_BINS as f64;
        let b = pos.floor() as usize % FOLD_BINS;
        let w = pos - pos.floor();
        bins[b] += f64::from(*v) * (1.0 - w);
        bins[(b + 1) % FOLD_BINS] += f64::from(*v) * w;
    }
    bins
}

/// Peak over mean of a fold.
fn sharpness(bins: &[f64; FOLD_BINS]) -> f64 {
    let mean = bins.iter().sum::<f64>() / FOLD_BINS as f64;
    let max = bins.iter().copied().fold(0.0, f64::max);
    if mean > 0.0 { max / mean } else { 0.0 }
}

/// Phase (0..1) of bin `top`, refined by parabolic interpolation (circular).
fn peak_phase(bins: &[f64; FOLD_BINS], top: usize) -> f64 {
    let l = bins[(top + FOLD_BINS - 1) % FOLD_BINS];
    let c = bins[top];
    let r = bins[(top + 1) % FOLD_BINS];
    let denom = l - 2.0 * c + r;
    let shift = if denom.abs() > f64::EPSILON {
        (0.5 * (l - r) / denom).clamp(-0.5, 0.5)
    } else {
        0.0
    };
    ((top as f64 + shift) / FOLD_BINS as f64).rem_euclid(1.0)
}

/// Sharpest fold within ±2 % of `coarse`, coarse-to-fine. Returns (bpm, sharpness).
fn refine(x: &[f32], rate: f64, coarse: f64) -> (f64, f64) {
    let mut best = (coarse, 0.0);
    let mut center = coarse;
    for (span, step) in [(0.02 * coarse, 0.05), (0.06, 0.005)] {
        let mut bpm = (center - span).max(MIN_BPM);
        while bpm <= (center + span).min(MAX_BPM) {
            let s = sharpness(&fold(x, rate, bpm));
            if s > best.1 {
                best = (bpm, s);
            }
            bpm += step;
        }
        center = best.0;
    }
    best
}

/// Where the beat sits within the period (0..1): of the strongest full-band peaks, the one with
/// the most low-band energy around it.
fn beat_phase(full: &[f32], low: &[f32], rate: f64, bpm: f64) -> f64 {
    let full_bins = fold(full, rate, bpm);
    let low_bins = fold(low, rate, bpm);
    let is_peak = |i: usize| {
        let c = full_bins[i];
        c > 0.0
            && c >= full_bins[(i + FOLD_BINS - 1) % FOLD_BINS]
            && c >= full_bins[(i + 1) % FOLD_BINS]
    };
    let mut peaks: Vec<usize> = (0..FOLD_BINS).filter(|&i| is_peak(i)).collect();
    peaks.sort_by(|a, b| full_bins[*b].total_cmp(&full_bins[*a]));
    peaks.truncate(PHASE_CANDIDATES);
    let low_around = |i: usize| -> f64 {
        (0..=4)
            .map(|d| low_bins[(i + FOLD_BINS + d - 2) % FOLD_BINS])
            .sum()
    };
    let best = peaks
        .iter()
        .copied()
        .max_by(|a, b| low_around(*a).total_cmp(&low_around(*b)))
        .filter(|&i| low_around(i) > 0.0)
        .or(peaks.first().copied())
        .unwrap_or(0);
    peak_phase(&full_bins, best)
}

#[cfg(test)]
mod tests {
    use super::super::onset::OnsetDetector;
    use super::super::synth::{Noise, Pattern, RATE};
    use super::*;

    fn analyse(samples: &[f32]) -> Option<BeatGrid> {
        let mut d = OnsetDetector::new(RATE);
        for chunk in samples.chunks(1000) {
            d.push(chunk);
        }
        estimate(&d.finish())
    }

    /// Tempo within ±0.05 BPM and the first beat within ±5 ms (of the true phase).
    fn check(pattern: Pattern) {
        check_as(pattern, pattern.bpm);
    }

    /// As `check`, for a pattern expected to be reported at `bpm` (an octave of its own).
    fn check_as(pattern: Pattern, bpm: f64) {
        let grid = analyse(&pattern.render())
            .unwrap_or_else(|| panic!("{} BPM: no tempo found", pattern.bpm));
        assert!(
            (grid.bpm - bpm).abs() <= 0.05,
            "{} BPM detected as {:.3}",
            pattern.bpm,
            grid.bpm
        );
        let period = 60.0 / bpm;
        let mut error = (grid.first_beat - pattern.first).rem_euclid(period);
        if error > period / 2.0 {
            error -= period;
        }
        assert!(
            error.abs() <= 0.005,
            "{} BPM: first beat off by {:.1} ms",
            pattern.bpm,
            error * 1000.0
        );
    }

    #[test]
    fn plain_kicks_at_dj_tempos() {
        for bpm in [90.0, 120.0, 128.0, 174.0] {
            check(Pattern::kicks(bpm, 60.0));
        }
    }

    #[test]
    fn full_pattern_with_snare_and_hats() {
        for bpm in [90.0, 120.0, 128.0, 174.0] {
            check(Pattern {
                hats: Some(0.5),
                snare: true,
                ..Pattern::kicks(bpm, 60.0)
            });
        }
    }

    #[test]
    fn swung_hats_do_not_shift_the_tempo() {
        for bpm in [90.0, 120.0] {
            check(Pattern {
                hats: Some(0.66),
                snare: true,
                ..Pattern::kicks(bpm, 60.0)
            });
        }
    }

    #[test]
    fn loud_open_hats_do_not_pull_the_grid_off_the_kick() {
        // House-style open hi-hats on the offbeat, louder than the kick in the full band.
        check(Pattern {
            hats: Some(0.5),
            hat_level: 0.9,
            hat_decay: 0.12,
            ..Pattern::kicks(124.0, 60.0)
        });
    }

    #[test]
    fn a_live_drummer_drifting_still_has_a_tempo() {
        // ±0.5 % tempo drift in slow 96-beat waves (a good drummer without click, like
        // Fleetwood Mac "Dreams"): the beat wanders ~0.08 beat against a constant grid between
        // parts of the track. Still a clear tempo, must not read as "unknown".
        let pattern = Pattern {
            drift: 0.005,
            drift_beats: 96.0,
            snare: true,
            hats: Some(0.5),
            ..Pattern::kicks(120.0, 90.0)
        };
        let grid = analyse(&pattern.render()).expect("tempo of a drifting band");
        assert!(
            (grid.bpm - 120.0).abs() < 1.0,
            "drifting 120 read as {}",
            grid.bpm
        );
    }

    #[test]
    fn a_breakdown_keeps_the_grid() {
        check(Pattern {
            gap: Some((32, 32)), // eight bars of silence in the middle
            hats: Some(0.5),
            ..Pattern::kicks(128.0, 60.0)
        });
    }

    #[test]
    fn odd_tempos_are_not_rounded() {
        check(Pattern::kicks(123.45, 60.0));
        check(Pattern::kicks(97.3, 60.0));
    }

    #[test]
    fn silence_has_no_tempo() {
        assert_eq!(analyse(&vec![0.0; RATE as usize * 30]), None);
    }

    #[test]
    fn noise_has_no_tempo() {
        for seed in [3, 11, 29, 101, 997] {
            let mut n = Noise::new(seed);
            let noise: Vec<f32> = (0..RATE as usize * 30).map(|_| 0.3 * n.next()).collect();
            assert_eq!(analyse(&noise), None, "seed {seed}");
        }
    }

    #[test]
    fn random_hits_have_no_tempo() {
        // Drum hits at random times: onsets, but no period.
        for seed in [5, 17, 41] {
            let mut n = Noise::new(seed);
            let mut out = vec![0.0f32; RATE as usize * 40];
            let mut t = 0usize;
            while t < out.len() {
                for (i, s) in out[t..].iter_mut().take(4000).enumerate() {
                    *s += 0.6 * n.next() * (-(i as f32) / 600.0).exp();
                }
                // 0.1–0.9 s apart, uniformly random.
                t += ((0.1 + 0.8 * (0.5 + 0.5 * n.next())) * RATE as f32) as usize;
            }
            assert_eq!(analyse(&out), None, "seed {seed}");
        }
    }

    #[test]
    fn tempos_land_in_the_dj_range() {
        assert_eq!(into_range(65.0), 130.0);
        assert_eq!(into_range(60.0), 120.0);
        assert_eq!(into_range(174.0), 174.0);
        assert_eq!(into_range(190.0), 95.0);
        assert_eq!(into_range(90.0), 90.0);
        check_as(Pattern::kicks(62.0, 60.0), 124.0);
    }

    #[test]
    fn too_short_has_no_tempo() {
        assert_eq!(analyse(&Pattern::kicks(120.0, 1.2).render()), None);
    }
}
