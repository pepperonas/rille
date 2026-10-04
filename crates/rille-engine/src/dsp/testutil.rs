//! Measurement helpers for DSP tests.

/// `seconds` of a unit sine at `freq`.
pub fn sine(freq: f32, sample_rate: u32, seconds: f32) -> Vec<f32> {
    let n = (sample_rate as f32 * seconds) as usize;
    (0..n)
        .map(|i| (i as f32 * freq * std::f32::consts::TAU / sample_rate as f32).sin())
        .collect()
}

fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|v| v * v).sum::<f32>() / x.len().max(1) as f32).sqrt()
}

/// Gain in dB of `output` relative to `input`, measured on the second half (after transients).
pub fn gain_db(input: &[f32], output: &[f32]) -> f32 {
    let half = input.len() / 2;
    20.0 * (rms(&output[half..]) / rms(&input[half..])).log10()
}
