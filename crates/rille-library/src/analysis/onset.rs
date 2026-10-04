//! Onset strength envelope by spectral flux: how much new energy appears from one short
//! frame to the next, summed over frequency. Peaks at drum hits and note starts.

use std::sync::Arc;

use realfft::num_complex::Complex;
use realfft::{RealFftPlanner, RealToComplex};

/// Log compression of magnitudes before differencing (Böck & Widmer): quiet and loud hits
/// count alike, so the envelope follows rhythm rather than loudness.
const COMPRESSION: f32 = 100.0;
/// Highest frequency that contributes; above this is mostly cymbal wash and noise.
const MAX_HZ: f32 = 10_000.0;
/// Upper edge of the low band: kick drums. In dance music the beat is where the kick is.
const LOW_HZ: f32 = 200.0;

/// Onset strength sampled at `rate` values per second, full band and low band. Value `i`
/// belongs to time `offset + i / rate` seconds.
#[derive(Debug, Clone)]
pub struct Envelope {
    pub full: Vec<f32>,
    pub low: Vec<f32>,
    pub rate: f64,
    pub offset: f64,
}

pub struct OnsetDetector {
    window: usize,
    hop: usize,
    sample_rate: u32,
    /// Ring of the last `window` samples.
    ring: Vec<f32>,
    /// Samples received so far.
    received: usize,
    /// Samples since the last frame.
    since_frame: usize,
    hann: Vec<f32>,
    fft: Arc<dyn RealToComplex<f32>>,
    input: Vec<f32>,
    spectrum: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,
    previous: Vec<f32>,
    current: Vec<f32>,
    bins: usize,
    low_bins: usize,
    full: Vec<f32>,
    low: Vec<f32>,
}

impl OnsetDetector {
    pub fn new(sample_rate: u32) -> OnsetDetector {
        // ~23 ms frames, ~5.8 ms hops at 44.1 kHz; scaled for other rates.
        let window = (sample_rate as f32 * 0.023).round().max(64.0) as usize;
        let window = window.next_power_of_two();
        let hop = window / 4;
        let fft = RealFftPlanner::<f32>::new().plan_fft_forward(window);
        let spectrum = fft.make_output_vec();
        let scratch = fft.make_scratch_vec();
        let bin_of =
            |hz: f32| ((hz / sample_rate as f32 * window as f32) as usize).min(spectrum.len());
        let bins = bin_of(MAX_HZ);
        let low_bins = bin_of(LOW_HZ).max(2);
        let hann = (0..window)
            .map(|i| {
                let x = i as f32 / window as f32;
                0.5 - 0.5 * (x * std::f32::consts::TAU).cos()
            })
            .collect();
        OnsetDetector {
            window,
            hop,
            sample_rate,
            ring: vec![0.0; window],
            received: 0,
            since_frame: 0,
            hann,
            input: vec![0.0; window],
            fft,
            spectrum,
            scratch,
            previous: vec![0.0; bins],
            current: vec![0.0; bins],
            bins,
            low_bins,
            full: Vec::new(),
            low: Vec::new(),
        }
    }

    pub fn push(&mut self, mono: &[f32]) {
        for &s in mono {
            self.ring[self.received % self.window] = s;
            self.received += 1;
            self.since_frame += 1;
            if self.since_frame == self.hop && self.received >= self.window {
                self.since_frame = 0;
                self.frame();
            } else if self.since_frame == self.hop {
                self.since_frame = 0;
            }
        }
    }

    fn frame(&mut self) {
        let start = self.received % self.window; // oldest sample in the ring
        for (i, x) in self.input.iter_mut().enumerate() {
            *x = self.ring[(start + i) % self.window] * self.hann[i];
        }
        if self
            .fft
            .process_with_scratch(&mut self.input, &mut self.spectrum, &mut self.scratch)
            .is_err()
        {
            return;
        }
        let (mut flux, mut low) = (0.0f32, 0.0f32);
        for (k, c) in self.spectrum[..self.bins].iter().enumerate() {
            let mag = (COMPRESSION * c.norm()).ln_1p();
            self.current[k] = mag;
            let rise = (mag - self.previous[k]).max(0.0);
            flux += rise;
            if k < self.low_bins {
                low += rise;
            }
        }
        std::mem::swap(&mut self.previous, &mut self.current);
        // The first frame has nothing to compare with.
        let first = self.full.is_empty();
        self.full.push(if first { 0.0 } else { flux });
        self.low.push(if first { 0.0 } else { low });
    }

    /// The envelope. Value `i` comes from the frame ending at sample `window + i·hop`; the
    /// time it stands for is the end of that frame's newest hop, where a fresh onset first
    /// raises the flux.
    pub fn finish(self) -> Envelope {
        let rate = f64::from(self.sample_rate) / self.hop as f64;
        let offset = (self.window - self.hop) as f64 / f64::from(self.sample_rate);
        Envelope {
            full: self.full,
            low: self.low,
            rate,
            offset,
        }
    }
}
