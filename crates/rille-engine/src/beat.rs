//! Beat arithmetic for sync, loops, beat jumps and hot cues. Pure functions, no allocation:
//! everything here runs on the audio thread.

use rille_core::{BeatGrid, TempoRange};

/// Beat position (in beats, fractional) of `frame` on `grid` at `sample_rate`.
pub fn beat_at(grid: &BeatGrid, frame: f64, sample_rate: u32) -> f64 {
    (frame / f64::from(sample_rate) - grid.first_beat) * grid.bpm / 60.0
}

/// Frame of beat position `beat` (inverse of [`beat_at`]).
pub fn frame_of_beat(grid: &BeatGrid, beat: f64, sample_rate: u32) -> f64 {
    (grid.first_beat + beat * 60.0 / grid.bpm) * f64::from(sample_rate)
}

/// Frames per beat at the track's own tempo (playback rate not applied: positions are in track
/// frames).
pub fn beat_frames(grid: &BeatGrid, sample_rate: u32) -> f64 {
    60.0 / grid.bpm * f64::from(sample_rate)
}

/// `x` folded into `[-0.5, 0.5)`: the shortest way round a beat.
pub fn wrap_half(x: f64) -> f64 {
    x - (x + 0.5).floor()
}

/// A grid the engine can work with.
pub fn usable(grid: &Option<BeatGrid>) -> Option<BeatGrid> {
    grid.filter(|g| g.bpm.is_finite() && g.bpm > 0.0 && g.first_beat.is_finite())
}

/// How to follow a tempo: the playback rate that brings `own_bpm` to `target_bpm`, after
/// choosing ×½, ×1 or ×2 of the target so the change is as small as possible (a 174 BPM track
/// follows an 87 BPM one at 174). `beats_per_target_beat` says how many own beats pass per
/// target beat (2 when following at double tempo).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Follow {
    pub rate: f64,
    pub beats_per_target_beat: f64,
}

pub fn follow(own_bpm: f64, target_bpm: f64) -> Option<Follow> {
    if !(own_bpm > 0.0 && target_bpm > 0.0) {
        return None;
    }
    [0.5, 1.0, 2.0]
        .into_iter()
        .map(|k| Follow {
            rate: target_bpm * k / own_bpm,
            beats_per_target_beat: k,
        })
        .min_by(|a, b| (a.rate.ln().abs()).total_cmp(&b.rate.ln().abs()))
}

/// Range and fader position that play at `rate`: the current range if it reaches, else the
/// narrowest one that does (finest resolution). `None` if not even ±50 % reaches.
pub fn tempo_for_rate(rate: f64, current: TempoRange) -> Option<(TempoRange, f32)> {
    let rate = rate as f32;
    if let Some(p) = current.position_for(rate) {
        return Some((current, p));
    }
    TempoRange::ALL
        .into_iter()
        .find_map(|r| r.position_for(rate).map(|p| (r, p)))
}

/// Phase error in own beats (`[-0.5, 0.5)`): positive means this deck is ahead of where it
/// should be.
pub fn phase_error(own_beat: f64, target_beat: f64, beats_per_target_beat: f64) -> f64 {
    wrap_half(own_beat - target_beat * beats_per_target_beat)
}

/// Nearest beat to `frame` (as a frame).
pub fn nearest_beat_frame(grid: &BeatGrid, frame: f64, sample_rate: u32) -> f64 {
    frame_of_beat(grid, beat_at(grid, frame, sample_rate).round(), sample_rate)
}

/// Where a new hot cue goes: onto the nearest beat if it is within ⅛ beat (the DJ meant the
/// beat), else exactly where the playhead is.
pub fn quantize_cue(grid: Option<BeatGrid>, frame: f64, sample_rate: u32) -> f64 {
    let Some(g) = grid else { return frame };
    let beat = beat_at(&g, frame, sample_rate);
    if (beat - beat.round()).abs() <= 0.125 {
        frame_of_beat(&g, beat.round(), sample_rate).max(0.0)
    } else {
        frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 48_000;
    const G: BeatGrid = BeatGrid {
        bpm: 120.0,
        first_beat: 0.25,
    };

    #[test]
    fn beats_and_frames_convert_both_ways() {
        assert_eq!(beat_at(&G, 0.25 * 48_000.0, SR), 0.0);
        assert_eq!(beat_at(&G, 1.25 * 48_000.0, SR), 2.0);
        assert_eq!(frame_of_beat(&G, 2.0, SR), 60_000.0);
        assert_eq!(beat_frames(&G, SR), 24_000.0);
        assert_eq!(beat_at(&G, 0.0, SR), -0.5);
    }

    #[test]
    fn wrap_takes_the_short_way() {
        for (x, w) in [
            (0.2, 0.2),
            (0.7, -0.3),
            (-0.7, 0.3),
            (3.4, 0.4),
            (-0.5, -0.5),
        ] {
            assert!((wrap_half(x) - w).abs() < 1e-12, "{x} → {}", wrap_half(x));
        }
    }

    #[test]
    fn following_picks_half_or_double_tempo_when_closer() {
        let f = follow(128.0, 126.0).unwrap();
        assert!((f.rate - 126.0 / 128.0).abs() < 1e-12 && f.beats_per_target_beat == 1.0);
        // Drum & bass at 174 next to a halftime 87: stays near its own speed.
        let f = follow(174.0, 87.5).unwrap();
        assert!((f.rate - 175.0 / 174.0).abs() < 1e-12 && f.beats_per_target_beat == 2.0);
        let f = follow(85.0, 172.0).unwrap();
        assert!((f.rate - 86.0 / 85.0).abs() < 1e-12 && f.beats_per_target_beat == 0.5);
        assert_eq!(follow(0.0, 120.0), None);
        assert_eq!(follow(120.0, f64::NAN), None);
    }

    #[test]
    fn tempo_keeps_the_range_when_it_reaches_else_widens() {
        let (r, p) = tempo_for_rate(1.05, TempoRange::Ten).unwrap();
        assert_eq!(r, TempoRange::Ten);
        assert!((p - 0.5).abs() < 1e-5);
        let (r, p) = tempo_for_rate(1.12, TempoRange::Six).unwrap();
        assert_eq!(r, TempoRange::Sixteen);
        assert!((p - 0.75).abs() < 1e-5);
        assert_eq!(tempo_for_rate(1.7, TempoRange::Ten), None);
    }

    #[test]
    fn phase_error_is_measured_in_own_beats() {
        assert!((phase_error(10.25, 4.0, 1.0) - 0.25).abs() < 1e-12);
        assert!((phase_error(9.9, 4.0, 1.0) + 0.1).abs() < 1e-12);
        // Following at double tempo: two own beats per target beat.
        assert!(phase_error(8.0, 4.0, 2.0).abs() < 1e-12);
        assert!((phase_error(8.5, 4.0, 2.0) + 0.5).abs() < 1e-12);
    }

    #[test]
    fn cues_snap_to_a_close_beat_only() {
        let beat2 = frame_of_beat(&G, 2.0, SR);
        assert_eq!(quantize_cue(Some(G), beat2 + 2_000.0, SR), beat2);
        assert_eq!(quantize_cue(Some(G), beat2 + 6_000.0, SR), beat2 + 6_000.0);
        assert_eq!(quantize_cue(None, 1_234.0, SR), 1_234.0);
        assert_eq!(nearest_beat_frame(&G, beat2 + 11_000.0, SR), beat2);
        assert_eq!(
            nearest_beat_frame(&G, beat2 + 13_000.0, SR),
            beat2 + 24_000.0
        );
    }

    #[test]
    fn broken_grids_are_not_used() {
        let bad = BeatGrid {
            bpm: 0.0,
            first_beat: 0.0,
        };
        assert_eq!(usable(&Some(bad)), None);
        assert_eq!(usable(&Some(G)), Some(G));
        assert_eq!(usable(&None), None);
    }
}
