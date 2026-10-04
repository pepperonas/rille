//! Fader and crossfader transfer curves.

use rille_core::CrossfaderCurve;
use std::f32::consts::FRAC_PI_2;

/// Width of the cut-in region at each end of the scratch curve (fraction of the travel).
pub const CUT_WIDTH: f32 = 0.05;

/// Channel fader position (0..1) to linear gain. Squared, so the upper half of the fader is
/// where the level changes musically (-12 dB at the middle), like a DJ mixer taper.
#[inline]
pub fn channel_gain(position: f32) -> f32 {
    let p = position.clamp(0.0, 1.0);
    p * p
}

/// Trim position (0..1) to linear gain, -12..+12 dB with 0.5 = unity.
#[inline]
pub fn trim_gain(position: f32) -> f32 {
    let db = (position.clamp(0.0, 1.0) - 0.5) * 24.0;
    10f32.powf(db / 20.0)
}

/// Crossfader position (0 = left/deck A, 1 = right/deck B) to (gain A, gain B).
#[inline]
pub fn crossfader_gains(position: f32, curve: CrossfaderCurve) -> (f32, f32) {
    let x = position.clamp(0.0, 1.0);
    match curve {
        CrossfaderCurve::Smooth => ((x * FRAC_PI_2).cos(), (x * FRAC_PI_2).sin()),
        CrossfaderCurve::Linear => (1.0 - x, x),
        CrossfaderCurve::Cut => (
            ((1.0 - x) / CUT_WIDTH).clamp(0.0, 1.0),
            (x / CUT_WIDTH).clamp(0.0, 1.0),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [CrossfaderCurve; 3] = [
        CrossfaderCurve::Smooth,
        CrossfaderCurve::Linear,
        CrossfaderCurve::Cut,
    ];

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn endpoints_are_exclusive_for_every_curve() {
        for curve in ALL {
            let (a, b) = crossfader_gains(0.0, curve);
            assert!(close(a, 1.0) && close(b, 0.0), "{curve:?} left");
            let (a, b) = crossfader_gains(1.0, curve);
            assert!(close(a, 0.0) && close(b, 1.0), "{curve:?} right");
        }
    }

    #[test]
    fn smooth_keeps_constant_power() {
        for i in 0..=100 {
            let (a, b) = crossfader_gains(i as f32 / 100.0, CrossfaderCurve::Smooth);
            assert!(close(a * a + b * b, 1.0));
        }
        let (a, b) = crossfader_gains(0.5, CrossfaderCurve::Smooth);
        assert!(close(a, std::f32::consts::FRAC_1_SQRT_2) && close(b, a));
    }

    #[test]
    fn linear_sums_to_one() {
        for i in 0..=100 {
            let (a, b) = crossfader_gains(i as f32 / 100.0, CrossfaderCurve::Linear);
            assert!(close(a + b, 1.0));
        }
    }

    #[test]
    fn cut_is_full_almost_everywhere() {
        for i in 5..=95 {
            let (a, b) = crossfader_gains(i as f32 / 100.0, CrossfaderCurve::Cut);
            assert!(close(a, 1.0) && close(b, 1.0), "at {i}%");
        }
        let (_, b) = crossfader_gains(0.025, CrossfaderCurve::Cut);
        assert!(close(b, 0.5));
    }

    #[test]
    fn out_of_range_positions_are_clamped() {
        for curve in ALL {
            assert_eq!(crossfader_gains(-1.0, curve), crossfader_gains(0.0, curve));
            assert_eq!(crossfader_gains(2.0, curve), crossfader_gains(1.0, curve));
        }
        assert_eq!(channel_gain(-0.5), 0.0);
        assert_eq!(channel_gain(1.5), 1.0);
    }

    #[test]
    fn channel_taper_is_minus_twelve_db_at_half() {
        let db = 20.0 * channel_gain(0.5).log10();
        assert!((db + 12.04).abs() < 0.1);
    }

    #[test]
    fn trim_is_unity_in_the_middle_and_twelve_db_at_the_ends() {
        assert!(close(trim_gain(0.5), 1.0));
        assert!((20.0 * trim_gain(1.0).log10() - 12.0).abs() < 1e-3);
        assert!((20.0 * trim_gain(0.0).log10() + 12.0).abs() < 1e-3);
    }
}
