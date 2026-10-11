//! Headphone output on a second device: the engine writes the headphone signal into a
//! lock-free ring, the second device's callback reads it through [`DriftReader`]. The two
//! devices run on separate clocks (±200 ppm is typical), so the reader resamples by a ratio a
//! PI controller derives from how full the ring is. Nothing here allocates or locks after
//! construction.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use rtrb::{Consumer, Producer, RingBuffer};

/// Ring capacity in stereo frames (≈ 340 ms at 48 kHz): far more than the target fill.
pub const CUE_RING_FRAMES: usize = 16_384;
/// Largest correction of the playback ratio (±0.5 %, far beyond any real clock drift).
const MAX_CORRECTION: f64 = 0.005;
/// Proportional gain on the fill error (relative to the target). The fill then moves back to
/// the target with τ = target / (rate · KP): ~10 s for 1024 frames at 48 kHz – slow enough to
/// be inaudible, fast enough for any clock drift. A drift d leaves a steady fill offset of
/// d / KP of the target (200 ppm → 10 %), which is harmless.
const KP: f64 = 0.002;
/// Smoothing of the measured fill per callback (~1 s): the block sawtooth must not steer the
/// ratio. Kept well below the control time constant, so the loop cannot oscillate.
const FILL_SMOOTHING: f64 = 0.01;
/// Integral time in P-loop time constants: removes the steady fill offset a large drift would
/// leave (1000 ppm would otherwise sit at half the target), slow enough not to swing.
const INTEGRAL_TAUS: f64 = 2.0;
/// Rate the integral time is measured against (exactness does not matter).
const NOMINAL_RATE: f64 = 48_000.0;

/// Engine and reader ends of the cue ring.
pub fn cue_ring() -> (Producer<f32>, Consumer<f32>) {
    RingBuffer::new(CUE_RING_FRAMES * 2)
}

/// Push one stereo frame; dropped (not blocking) when the ring is full.
#[inline]
pub fn push_frame(tx: &mut Producer<f32>, l: f32, r: f32) -> bool {
    if tx.slots() < 2 {
        return false;
    }
    let _ = tx.push(l);
    let _ = tx.push(r);
    true
}

/// Where the reader takes frames from (the ring in the app, a model in tests).
pub trait FrameSource {
    fn frames_available(&self) -> usize;
    fn pop_frame(&mut self) -> Option<[f32; 2]>;
    /// Throw everything away (after the device was switched, the backlog is stale).
    fn clear(&mut self);
}

impl FrameSource for Consumer<f32> {
    fn frames_available(&self) -> usize {
        self.slots() / 2
    }

    fn pop_frame(&mut self) -> Option<[f32; 2]> {
        if self.slots() < 2 {
            return None;
        }
        Some([self.pop().ok()?, self.pop().ok()?])
    }

    /// Whole frames only: the engine may be between the left and the right sample of a frame,
    /// and dropping a lone left would swap the channels for good.
    fn clear(&mut self) {
        while self.slots() >= 2 {
            let _ = self.pop();
            let _ = self.pop();
        }
    }
}

/// Reads the ring at a slightly corrected speed so its fill stays at `target` frames.
pub struct DriftReader<S: FrameSource> {
    source: S,
    /// Last four input frames (cubic interpolation between [1] and [2]).
    hist: [[f32; 2]; 4],
    /// Position between hist[1] and hist[2], 0..1.
    frac: f64,
    target: f64,
    fill: f64,
    ratio: f64,
    /// Waiting for the ring to fill before playing (start, or after running dry).
    priming: bool,
    /// Sum of fill error × seconds (integral action).
    integral: f64,
    /// Shared with the control side: read without touching the audio callback's slot.
    underruns: Arc<AtomicU32>,
}

impl<S: FrameSource> DriftReader<S> {
    /// `target`: frames to keep in the ring; about two blocks of the slower side.
    pub fn new(source: S, target: usize) -> DriftReader<S> {
        DriftReader {
            source,
            hist: [[0.0; 2]; 4],
            frac: 0.0,
            target: target.max(64) as f64,
            fill: target as f64,
            ratio: 1.0,
            priming: true,
            integral: 0.0,
            underruns: Arc::new(AtomicU32::new(0)),
        }
    }

    /// Dropouts since the last restart, readable from any thread.
    pub fn underrun_counter(&self) -> Arc<AtomicU32> {
        self.underruns.clone()
    }

    /// Start over (new device): drop the backlog and fill up again.
    pub fn restart(&mut self, target: usize) {
        self.source.clear();
        self.hist = [[0.0; 2]; 4];
        self.frac = 0.0;
        self.target = target.max(64) as f64;
        self.fill = self.target;
        self.ratio = 1.0;
        self.priming = true;
        self.integral = 0.0;
        self.underruns.store(0, Ordering::Relaxed);
    }

    /// Current input/output ratio (1 + correction).
    pub fn ratio(&self) -> f64 {
        self.ratio
    }

    pub fn underruns(&self) -> u32 {
        self.underruns.load(Ordering::Relaxed)
    }

    pub fn fill(&self) -> usize {
        self.source.frames_available()
    }

    /// Fill `out` (interleaved, `channels` per frame; the signal goes to the first two).
    pub fn render(&mut self, out: &mut [f32], channels: usize) {
        out.fill(0.0);
        if channels == 0 {
            return;
        }
        let mut available = self.source.frames_available() as f64;
        if self.priming {
            if available < self.target {
                return;
            }
            // Start at the target, not with whatever piled up meanwhile (a device switch can
            // leave hundreds of milliseconds): that backlog would otherwise take the ±0.5 %
            // correction a minute to drain, all of it extra headphone latency.
            while available > self.target + 1.0 && self.source.pop_frame().is_some() {
                available -= 1.0;
            }
            self.priming = false;
            self.fill = available;
        }
        let frames = (out.len() / channels) as f64;
        self.fill += FILL_SMOOTHING * (available - self.fill);
        // Relative to the target: with big blocks the measured fill jumps by a whole block as
        // the two devices' block phases pass each other, and that must not swing the ratio.
        // The loop is then slower (τ = target / (rate · KP)); the integral follows at 2 τ.
        let error = (self.fill - self.target) / self.target;
        let integral_seconds = INTEGRAL_TAUS * self.target / (NOMINAL_RATE * KP);
        let limit = MAX_CORRECTION / KP * integral_seconds;
        self.integral = (self.integral + error * frames / NOMINAL_RATE).clamp(-limit, limit);
        let correction = KP * (error + self.integral / integral_seconds);
        self.ratio = 1.0 + correction.clamp(-MAX_CORRECTION, MAX_CORRECTION);

        for frame in out.chunks_exact_mut(channels) {
            self.frac += self.ratio;
            while self.frac >= 1.0 {
                let Some(next) = self.source.pop_frame() else {
                    // Ran dry: silence for the rest, then wait for the ring to fill again.
                    self.underruns.fetch_add(1, Ordering::Relaxed);
                    self.priming = true;
                    self.frac = 0.0;
                    return;
                };
                self.hist = [self.hist[1], self.hist[2], self.hist[3], next];
                self.frac -= 1.0;
            }
            let t = self.frac as f32;
            for (c, sample) in frame.iter_mut().take(2).enumerate() {
                *sample = cubic(
                    self.hist[0][c],
                    self.hist[1][c],
                    self.hist[2][c],
                    self.hist[3][c],
                    t,
                );
            }
        }
    }
}

/// 4-point Hermite between `b` and `c`.
#[inline]
fn cubic(a: f32, b: f32, c: f32, d: f32, t: f32) -> f32 {
    let c0 = b;
    let c1 = 0.5 * (c - a);
    let c2 = a - 2.5 * b + 2.0 * c - 0.5 * d;
    let c3 = 0.5 * (d - a) + 1.5 * (b - c);
    ((c3 * t + c2) * t + c1) * t + c0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    /// A ring model with a sine in it, so continuity can be checked.
    struct Model {
        frames: VecDeque<[f32; 2]>,
        phase: f64,
    }

    impl Model {
        fn produce(&mut self, n: usize) {
            for _ in 0..n {
                let v = (self.phase * 0.01).sin() as f32;
                self.frames.push_back([v, -v]);
                self.phase += 1.0;
            }
        }
    }

    impl FrameSource for Model {
        fn frames_available(&self) -> usize {
            self.frames.len()
        }
        fn pop_frame(&mut self) -> Option<[f32; 2]> {
            self.frames.pop_front()
        }
        fn clear(&mut self) {
            self.frames.clear();
        }
    }

    /// Reader, fill range, largest sample step, ratio (min, mean, max).
    type Simulation = (DriftReader<Model>, (usize, usize), f32, (f64, f64, f64));

    /// Engine blocks of `engine_block` at its clock, device callbacks of `device_block` at a
    /// clock off by `ppm`. Returns the reader after `seconds`, the fill range seen after the
    /// first `settle` seconds, and the largest sample step (a click shows as a jump).
    fn simulate(
        ppm: f64,
        engine_block: usize,
        device_block: usize,
        seconds: f64,
        settle: f64,
    ) -> Simulation {
        let sr = 48_000.0;
        let model = Model {
            frames: VecDeque::new(),
            phase: 0.0,
        };
        let mut reader = DriftReader::new(model, 2 * engine_block.max(device_block));
        let engine_period = engine_block as f64 / sr;
        let device_period = device_block as f64 / (sr * (1.0 + ppm * 1e-6));
        let (mut t_engine, mut t_device) = (0.0, 0.0);
        let mut out = vec![0.0f32; device_block * 2];
        let mut range = (usize::MAX, 0);
        let mut last = 0.0f32;
        let mut max_step = 0.0f32;
        let mut ratios = (f64::MAX, 0.0f64);
        let (mut sum, mut n) = (0.0f64, 0usize);
        while t_device < seconds {
            if t_engine <= t_device {
                reader.source.produce(engine_block);
                t_engine += engine_period;
            } else {
                reader.render(&mut out, 2);
                if t_device > settle {
                    let fill = reader.fill();
                    range = (range.0.min(fill), range.1.max(fill));
                    ratios = (ratios.0.min(reader.ratio()), ratios.1.max(reader.ratio()));
                    sum += reader.ratio();
                    n += 1;
                    for f in out.as_chunks::<2>().0 {
                        max_step = max_step.max((f[0] - last).abs());
                        last = f[0];
                    }
                } else if let Some(f) = out.as_chunks::<2>().0.last() {
                    last = f[0];
                }
                t_device += device_period;
            }
        }
        (
            reader,
            range,
            max_step,
            (ratios.0, sum / n.max(1) as f64, ratios.1),
        )
    }

    /// The average ratio matches the drift; it wanders (the block phase between the two
    /// devices drifts too) but stays far below anything audible (1 cent ≈ 0.06 %).
    fn assert_follows(ratios: (f64, f64, f64), expected: f64) {
        let (lo, mean, hi) = ratios;
        assert!(
            (mean - expected).abs() < 0.000_02,
            "mean {mean}, expected {expected}"
        );
        assert!(
            hi - expected < 0.000_6 && expected - lo < 0.000_6,
            "{lo}..{hi}"
        );
    }

    #[test]
    fn a_fast_device_is_followed_without_dropouts() {
        // The device plays 200 ppm fast: it takes more frames than the engine makes, so the
        // reader must read slower (input per output frame 1 / 1.0002).
        let (r, (lo, hi), step, ratios) = simulate(200.0, 256, 512, 180.0, 60.0);
        assert_eq!(r.underruns(), 0, "fill {lo}..{hi}, ratio {}", r.ratio());
        assert_follows(ratios, 1.0 / 1.000_2);
        // Measured right after each read: never close to running dry, never near the capacity.
        assert!(lo > 128 && hi < 2_048, "fill {lo}..{hi}");
        // The sine advances 0.01 rad per frame: steps stay near 0.01 (no jumps, no repeats).
        assert!(step < 0.015, "largest step {step}");
    }

    #[test]
    fn a_slow_device_is_followed_without_the_ring_overflowing() {
        let (r, (lo, hi), _, ratios) = simulate(-200.0, 512, 256, 180.0, 60.0);
        assert_eq!(r.underruns(), 0);
        assert_follows(ratios, 1.0 / 0.999_8);
        assert!(lo > 128 && hi < 2_048, "fill {lo}..{hi}");
    }

    #[test]
    fn a_large_drift_is_followed_too() {
        // 1000 ppm (a cheap Bluetooth clock): without integral action the fill would sit far
        // below the target and run dry now and then.
        let (r, (lo, _), _, ratios) = simulate(1_000.0, 256, 512, 300.0, 120.0);
        assert_eq!(r.underruns(), 0);
        assert!(lo > 128, "fill down to {lo}");
        assert!(
            (ratios.1 - 1.0 / 1.001).abs() < 0.000_05,
            "mean {}",
            ratios.1
        );
    }

    #[test]
    fn stable_with_very_different_block_sizes() {
        // No dropouts and an inaudible speed wobble (< 0.1 % ≈ 1.7 cents) for odd pairings.
        // With equal big blocks the two block phases pass each other slowly (every ~140 s at
        // 300 ppm), the measured fill makes a slow 2048-frame sawtooth and the ratio follows
        // it: that is the real fill moving, not the loop swinging.
        for (engine, device, ppm) in [
            (64, 2_048, 200.0),
            (1_024, 64, -200.0),
            (2_048, 2_048, 300.0),
        ] {
            let (r, (lo, _), step, (min, _, max)) = simulate(ppm, engine, device, 360.0, 180.0);
            let expected = 1.0 / (1.0 + ppm * 1e-6);
            assert_eq!(r.underruns(), 0, "{engine}/{device}");
            assert!(lo > 0, "{engine}/{device}: fill down to {lo}");
            assert!(
                (min - expected).abs() < 0.001 && (max - expected).abs() < 0.001,
                "{engine}/{device}: ratio {min}..{max}"
            );
            assert!(step < 0.015, "{engine}/{device}: step {step}");
        }
    }

    #[test]
    fn a_backlog_is_dropped_when_playing_starts() {
        let model = Model {
            frames: VecDeque::new(),
            phase: 0.0,
        };
        let mut r = DriftReader::new(model, 1_024);
        r.source.produce(15_000); // a device switch left 300 ms in the ring
        let mut out = vec![0.0f32; 256 * 2];
        r.render(&mut out, 2);
        assert!(
            r.fill() <= 1_024,
            "starts at the target, not 300 ms late: {}",
            r.fill()
        );
    }

    #[test]
    fn running_dry_waits_for_a_full_ring_before_playing_again() {
        let model = Model {
            frames: VecDeque::new(),
            phase: 0.0,
        };
        let mut r = DriftReader::new(model, 1_024);
        let mut out = vec![1.0f32; 256 * 2];
        r.source.produce(512);
        r.render(&mut out, 2);
        assert!(
            out.iter().all(|s| *s == 0.0),
            "below the target: silent, still priming"
        );
        r.source.produce(600);
        r.render(&mut out, 2);
        assert!(out.iter().any(|s| *s != 0.0), "primed: playing");
        // Drain it: an underrun is counted, the reader primes again.
        for _ in 0..8 {
            r.render(&mut out, 2);
        }
        assert_eq!(r.underruns(), 1);
        r.source.produce(100);
        r.render(&mut out, 2);
        assert!(
            out.iter().all(|s| *s == 0.0),
            "waits for a full ring, no stutter"
        );
    }

    #[test]
    fn extra_channels_stay_silent_and_restart_drops_the_backlog() {
        let model = Model {
            frames: VecDeque::new(),
            phase: 100.0,
        };
        let mut r = DriftReader::new(model, 64);
        r.source.produce(1_000);
        let mut out = vec![9.0f32; 64 * 4];
        r.render(&mut out, 4);
        assert!(
            out.as_chunks::<4>()
                .0
                .iter()
                .all(|f| f[2] == 0.0 && f[3] == 0.0)
        );
        r.restart(128);
        assert_eq!(r.fill(), 0);
    }

    #[test]
    fn clearing_never_splits_a_frame() {
        // The engine has pushed the left sample of a frame but not yet the right one.
        let (mut tx, mut rx) = cue_ring();
        assert!(tx.push(0.1).is_ok());
        rx.clear();
        assert!(tx.push(0.2).is_ok());
        assert_eq!(rx.pop_frame(), Some([0.1, 0.2]), "left stays left");
    }
}
