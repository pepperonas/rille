//! The playhead of one deck: where in the track we are and how fast we move.
//!
//! - **Varispeed** (pitch follows tempo): 4-point Hermite interpolation at a fractional
//!   playhead; also used for reverse and scratching.
//! - **Keylock** (pitch stays): Signalsmith Stretch, fed with `rate × frames` input frames per
//!   block. The feed runs ahead of the playhead by the stretcher's latency so what you hear
//!   lines up with the playhead; after a jump the stretcher is reset and pre-rolled.
//! - **Scratch:** while the jog platter is touched (vinyl mode) its speed drives the playhead.
//! - **Bend:** the jog rim speeds up or slows down temporarily; app buttons hold a fixed bend.
//!
//! Everything here runs on the audio thread: no allocation after `new`, verified for the C++
//! stretcher by `tests/stretch_alloc.rs` and for the rest by `tests/no_alloc.rs`.

use rille_core::{TempoRange, TrackAudio};
use signalsmith_stretch::Stretch;

use crate::smooth::Smoother;

/// Jog platter resolution in ticks per revolution (DDJ-200). An assumption until calibrated on
/// the hardware: it only scales how far a scratch moves the record.
pub const JOG_TICKS_PER_REV: f32 = 2048.0;
/// A record at 33⅓ rpm.
pub const PLATTER_REV_PER_SEC: f32 = 100.0 / 3.0 / 60.0;
/// Rate change per (tick/second) of the jog rim.
pub const BEND_PER_TICK_RATE: f32 = 0.0005;
/// Bend while an app bend button is held.
pub const BEND_HOLD: f32 = 0.04;
pub const MAX_BEND: f32 = 0.5;
/// Seconds moved per search tick (shift + platter).
pub const SEARCH_SECONDS_PER_TICK: f64 = 0.05;
/// Highest playback rate the keylock input buffer is sized for (wide range plus bend).
const MAX_KEYLOCK_RATE: f32 = 2.25;
const TEMPO_SMOOTHING_MS: f32 = 30.0;
const SCRATCH_SMOOTHING_MS: f32 = 6.0;
const BEND_SMOOTHING_MS: f32 = 60.0;
/// Frames over which the varispeed bridge hands over to the keylock stretcher.
const KEYLOCK_CROSSFADE: usize = 256;

/// What happened at the track boundaries during a render.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Boundaries {
    /// Reached the end moving forward.
    pub end: bool,
    /// Reached the start moving backward.
    pub start: bool,
}

pub struct Player {
    sample_rate: u32,
    playhead: f64,
    tempo: f32,
    range: TempoRange,
    keylock: bool,
    reverse: bool,
    scratching: bool,
    /// Rate without bend: tempo/direction, or the platter while scratching.
    rate: Smoother,
    bend: Smoother,
    bend_ticks: i32,
    bend_hold: i8,
    scratch_ticks: i32,
    was_playing: bool,
    last_rate: f32,
    stretch: Stretch,
    stretch_in: Vec<f32>,
    /// Input handed to `seek` after a jump: one full analysis block before the feed point.
    /// Separate from `stretch_in`, which only holds one audio block.
    preroll: Vec<f32>,
    stretch_out: Vec<f32>,
    keylock_engaged: bool,
    feed: f64,
    /// Output frames since the stretcher was (re)engaged. Until its pre-roll has passed, a
    /// varispeed rendering bridges the gap and then crossfades into the stretched signal.
    since_engage: usize,
    bridge_pos: f64,
}

#[inline]
fn hermite(xm1: f32, x0: f32, x1: f32, x2: f32, t: f32) -> f32 {
    let c1 = 0.5 * (x1 - xm1);
    let c2 = xm1 - 2.5 * x0 + 2.0 * x1 - 0.5 * x2;
    let c3 = 0.5 * (x2 - xm1) + 1.5 * (x0 - x1);
    ((c3 * t + c2) * t + c1) * t + x0
}

/// Stereo frame at a fractional position (silence outside the track).
#[inline]
pub fn sample_at(track: &TrackAudio, position: f64) -> [f32; 2] {
    let i = position.floor();
    let t = (position - i) as f32;
    let i = i as i64;
    let at = |k: i64| {
        if k < 0 {
            [0.0; 2]
        } else {
            track.frame(k as usize)
        }
    };
    let (a, b, c, d) = (at(i - 1), at(i), at(i + 1), at(i + 2));
    [
        hermite(a[0], b[0], c[0], d[0], t),
        hermite(a[1], b[1], c[1], d[1], t),
    ]
}

impl Player {
    pub fn new(sample_rate: u32, max_block: usize) -> Player {
        let capacity = (max_block as f32 * MAX_KEYLOCK_RATE).ceil() as usize + 4;
        let stretch = Stretch::preset_default(2, sample_rate);
        let preroll_frames = stretch.input_latency() * 2;
        Player {
            sample_rate,
            playhead: 0.0,
            tempo: 0.0,
            range: TempoRange::default(),
            keylock: false,
            reverse: false,
            scratching: false,
            rate: Smoother::new(sample_rate, TEMPO_SMOOTHING_MS, 0.0),
            bend: Smoother::new(sample_rate, BEND_SMOOTHING_MS, 0.0),
            bend_ticks: 0,
            bend_hold: 0,
            scratch_ticks: 0,
            was_playing: false,
            last_rate: 0.0,
            stretch,
            stretch_in: vec![0.0; capacity * 2],
            preroll: vec![0.0; preroll_frames * 2],
            stretch_out: vec![0.0; max_block * 2],
            keylock_engaged: false,
            feed: 0.0,
            since_engage: 0,
            bridge_pos: 0.0,
        }
    }

    pub fn playhead(&self) -> f64 {
        self.playhead
    }

    pub fn tempo(&self) -> f32 {
        self.tempo
    }

    pub fn range(&self) -> TempoRange {
        self.range
    }

    pub fn keylock(&self) -> bool {
        self.keylock
    }

    pub fn reverse(&self) -> bool {
        self.reverse
    }

    pub fn scratching(&self) -> bool {
        self.scratching
    }

    /// Effective rate of the last rendered sample (negative = backwards).
    pub fn rate(&self) -> f32 {
        self.last_rate
    }

    /// The rate the tempo fader asks for, ignoring bend.
    pub fn tempo_rate(&self) -> f32 {
        self.range.rate(self.tempo)
    }

    /// New track or a jump (cue, seek): continue from `position`.
    pub fn jump_to(&mut self, position: f64) {
        self.playhead = position.max(0.0);
        self.keylock_engaged = false;
    }

    /// Fresh state for a newly loaded track (tempo, range and keylock stay, like on a CDJ).
    pub fn reset_for_track(&mut self) {
        self.jump_to(0.0);
        self.scratching = false;
        self.reverse = false;
        self.scratch_ticks = 0;
        self.bend_ticks = 0;
        self.bend.set_target(0.0);
        self.bend.snap();
    }

    pub fn set_tempo(&mut self, position: f32) {
        self.tempo = if position.is_finite() {
            position.clamp(-1.0, 1.0)
        } else {
            0.0
        };
    }

    pub fn set_range(&mut self, range: TempoRange) {
        self.range = range;
    }

    pub fn cycle_range(&mut self) {
        self.range = self.range.next();
    }

    pub fn set_keylock(&mut self, on: bool) {
        self.keylock = on;
        self.keylock_engaged = false;
    }

    pub fn bend(&mut self, ticks: i32) {
        self.bend_ticks = self.bend_ticks.saturating_add(ticks);
    }

    pub fn bend_hold(&mut self, direction: i8) {
        self.bend_hold = direction.signum();
    }

    pub fn scratch_touch(&mut self, touching: bool) {
        self.scratching = touching;
        self.scratch_ticks = 0;
        self.keylock_engaged = false;
        self.rate.set_time(
            self.sample_rate,
            if touching {
                SCRATCH_SMOOTHING_MS
            } else {
                TEMPO_SMOOTHING_MS
            },
        );
    }

    pub fn scratch(&mut self, ticks: i32) {
        if self.scratching {
            self.scratch_ticks = self.scratch_ticks.saturating_add(ticks);
        }
    }

    pub fn search(&mut self, ticks: i32, frames: u64) {
        let delta = f64::from(ticks) * SEARCH_SECONDS_PER_TICK * f64::from(self.sample_rate);
        self.jump_to((self.playhead + delta).clamp(0.0, frames as f64));
    }

    pub fn set_reverse(&mut self, on: bool) {
        self.reverse = on;
        self.keylock_engaged = false;
    }

    fn update_targets(&mut self, playing: bool, frames: usize) {
        let dt = frames as f32 / self.sample_rate as f32;
        if self.scratching {
            let ticks_per_sec = self.scratch_ticks as f32 / dt;
            self.rate
                .set_target(ticks_per_sec / (JOG_TICKS_PER_REV * PLATTER_REV_PER_SEC));
            self.scratch_ticks = 0;
            self.bend.set_target(0.0);
            self.bend_ticks = 0;
            return;
        }
        let base = if playing {
            self.tempo_rate() * if self.reverse { -1.0 } else { 1.0 }
        } else {
            0.0
        };
        self.rate.set_target(base);
        // Start and stop are instant (the declick handles the edge); only tempo moves glide.
        if playing != self.was_playing {
            self.rate.snap();
        }
        let ticks_per_sec = self.bend_ticks as f32 / dt;
        self.bend_ticks = 0;
        let bend = ticks_per_sec * BEND_PER_TICK_RATE + f32::from(self.bend_hold) * BEND_HOLD;
        self.bend.set_target(if playing {
            bend.clamp(-MAX_BEND, MAX_BEND)
        } else {
            0.0
        });
    }

    /// Render `out.len() / 2` stereo frames.
    pub fn render(&mut self, track: &TrackAudio, playing: bool, out: &mut [f32]) -> Boundaries {
        let frames = out.len() / 2;
        self.update_targets(playing, frames);
        self.was_playing = playing;
        let use_keylock = self.keylock
            && playing
            && !self.scratching
            && !self.reverse
            && self.rate.target() > 0.0;
        if use_keylock {
            self.render_keylock(track, out)
        } else {
            self.keylock_engaged = false;
            self.render_varispeed(track, out)
        }
    }

    fn render_varispeed(&mut self, track: &TrackAudio, out: &mut [f32]) -> Boundaries {
        let end = track.frames() as f64;
        let mut hit = Boundaries::default();
        for frame in out.as_chunks_mut::<2>().0 {
            let rate = self.rate.tick() * (1.0 + self.bend.tick());
            self.last_rate = rate;
            if rate == 0.0 {
                *frame = [0.0; 2];
                continue;
            }
            *frame = sample_at(track, self.playhead);
            self.playhead += f64::from(rate);
            if self.playhead >= end {
                self.playhead = end;
                hit.end = rate > 0.0;
            } else if self.playhead <= 0.0 {
                self.playhead = 0.0;
                hit.start = rate < 0.0;
            }
        }
        hit
    }

    fn engage_keylock(&mut self, track: &TrackAudio, rate: f32) {
        self.stretch.reset();
        let input_latency = self.stretch.input_latency() as f64;
        let output_latency = self.stretch.output_latency() as f64;
        // Feed ahead so the stretched output lines up with the playhead.
        self.feed = self.playhead + input_latency + output_latency * f64::from(rate);
        // Pre-roll with the input just before the feed point, so there is no silent gap.
        let preroll = self.preroll.len() / 2;
        let start = self.feed.floor() as i64 - preroll as i64;
        for (k, frame) in self.preroll.as_chunks_mut::<2>().0.iter_mut().enumerate() {
            let i = start + k as i64;
            *frame = if i < 0 {
                [0.0; 2]
            } else {
                track.frame(i as usize)
            };
        }
        self.stretch.seek(&self.preroll, f64::from(rate));
        self.keylock_engaged = true;
        self.since_engage = 0;
        self.bridge_pos = self.playhead;
    }

    fn render_keylock(&mut self, track: &TrackAudio, out: &mut [f32]) -> Boundaries {
        let frames = out.len() / 2;
        if !self.keylock_engaged {
            let rate = self.rate.current() * (1.0 + self.bend.current());
            self.engage_keylock(track, rate);
        }
        let bridge_until = self.stretch.output_latency() + KEYLOCK_CROSSFADE;
        let bridging = self.since_engage < bridge_until;
        // Per-sample rates: the stretcher takes their sum, the bridge uses them directly.
        let mut advance = 0.0f64;
        let mut rate = 0.0;
        for frame in out.as_chunks_mut::<2>().0 {
            rate = self.rate.tick() * (1.0 + self.bend.tick());
            advance += f64::from(rate);
            if bridging {
                *frame = sample_at(track, self.bridge_pos);
                self.bridge_pos += f64::from(rate);
            }
        }
        self.last_rate = rate;

        let first = self.feed.floor() as i64;
        let wanted = ((self.feed + advance).floor() as i64 - first).max(0) as usize;
        let n_in = wanted.min(self.stretch_in.len() / 2);
        for (k, frame) in self.stretch_in[..n_in * 2]
            .as_chunks_mut::<2>()
            .0
            .iter_mut()
            .enumerate()
        {
            let i = first + k as i64;
            *frame = if i < 0 {
                [0.0; 2]
            } else {
                track.frame(i as usize)
            };
        }
        let stretched = &mut self.stretch_out[..frames * 2];
        self.stretch
            .process(&self.stretch_in[..n_in * 2], &mut *stretched);

        if bridging {
            let start = self.stretch.output_latency();
            for (k, (o, s)) in out
                .as_chunks_mut::<2>()
                .0
                .iter_mut()
                .zip(stretched.as_chunks::<2>().0)
                .enumerate()
            {
                let pos = self.since_engage + k;
                let w = (pos.saturating_sub(start) as f32 / KEYLOCK_CROSSFADE as f32).min(1.0);
                o[0] += w * (s[0] - o[0]);
                o[1] += w * (s[1] - o[1]);
            }
        } else {
            out.copy_from_slice(stretched);
        }
        self.since_engage = self.since_engage.saturating_add(frames);
        self.feed += advance;
        self.playhead += advance;
        let end = track.frames() as f64;
        let mut hit = Boundaries::default();
        if self.playhead >= end {
            self.playhead = end;
            hit.end = true;
        }
        hit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 48_000;

    fn sine_track(freq: f32, seconds: f32) -> TrackAudio {
        let n = (seconds * SR as f32) as usize;
        let samples = (0..n)
            .flat_map(|i| {
                let v = 0.5 * (i as f32 * freq * std::f32::consts::TAU / SR as f32).sin();
                [v, v]
            })
            .collect();
        TrackAudio::new(1, SR, samples)
    }

    /// Render `seconds` and return the left channel.
    fn play(p: &mut Player, track: &TrackAudio, playing: bool, seconds: f32) -> Vec<f32> {
        let mut out = vec![0.0; 256 * 2];
        let mut left = Vec::new();
        for _ in 0..(seconds * SR as f32 / 256.0) as usize {
            p.render(track, playing, &mut out);
            left.extend(out.as_chunks::<2>().0.iter().map(|f| f[0]));
        }
        left
    }

    /// Frequency from rising zero crossings over the second half of the signal.
    fn frequency(v: &[f32]) -> f32 {
        let body = &v[v.len() / 2..];
        let crossings = body
            .windows(2)
            .filter(|w| w[0] < 0.0 && w[1] >= 0.0)
            .count();
        crossings as f32 * SR as f32 / body.len() as f32
    }

    #[test]
    fn hermite_hits_the_samples_exactly() {
        let t = TrackAudio::new(1, SR, vec![0.1, 0.1, 0.5, 0.5, -0.3, -0.3, 0.2, 0.2]);
        assert_eq!(sample_at(&t, 1.0), [0.5, 0.5]);
        assert_eq!(sample_at(&t, 2.0), [-0.3, -0.3]);
        assert_eq!(sample_at(&t, 10.0), [0.0, 0.0]);
    }

    #[test]
    fn normal_speed_reproduces_the_track() {
        let track = sine_track(440.0, 2.0);
        let mut p = Player::new(SR, 256);
        let out = play(&mut p, &track, true, 1.0);
        for (i, v) in out.iter().take(1000).enumerate() {
            assert!((v - track.frame(i)[0]).abs() < 1e-6, "sample {i}");
        }
        assert_eq!(p.rate(), 1.0);
    }

    #[test]
    fn tempo_up_plays_faster_and_higher_without_keylock() {
        let track = sine_track(440.0, 6.0);
        let mut p = Player::new(SR, 256);
        p.set_range(TempoRange::Ten);
        p.set_tempo(1.0); // +10 %
        let out = play(&mut p, &track, true, 2.0);
        let expected = 2.0 * 1.1 * SR as f64;
        assert!(
            (p.playhead() - expected).abs() < 0.01 * expected,
            "{}",
            p.playhead()
        );
        let f = frequency(&out);
        assert!((f - 484.0).abs() < 5.0, "pitch follows tempo: {f}");
    }

    #[test]
    fn keylock_keeps_the_pitch_and_the_tempo() {
        let track = sine_track(440.0, 8.0);
        let mut p = Player::new(SR, 256);
        p.set_range(TempoRange::Sixteen);
        p.set_tempo(1.0); // +16 %
        p.set_keylock(true);
        let out = play(&mut p, &track, true, 3.0);
        let expected = 3.0 * 1.16 * SR as f64;
        assert!(
            (p.playhead() - expected).abs() < 0.01 * expected,
            "{}",
            p.playhead()
        );
        let f = frequency(&out);
        assert!((f - 440.0).abs() < 4.4, "pitch stays at 440 Hz: {f}");
    }

    #[test]
    fn keylock_slower_keeps_the_pitch_too() {
        let track = sine_track(440.0, 6.0);
        let mut p = Player::new(SR, 256);
        p.set_range(TempoRange::Sixteen);
        p.set_tempo(-1.0); // -16 %
        p.set_keylock(true);
        let f = frequency(&play(&mut p, &track, true, 3.0));
        assert!((f - 440.0).abs() < 4.4, "{f}");
    }

    #[test]
    fn keylock_has_no_silent_gap_after_a_jump() {
        let track = sine_track(440.0, 8.0);
        let mut p = Player::new(SR, 256);
        p.set_keylock(true);
        play(&mut p, &track, true, 1.0);
        p.jump_to(3.0 * SR as f64);
        // Long enough to cover the varispeed bridge AND the hand-over to the stretcher.
        let out = play(&mut p, &track, true, 0.3);
        let quiet = out
            .chunks(256)
            .filter(|b| b.iter().all(|v| v.abs() < 0.05))
            .count();
        assert_eq!(quiet, 0, "no silent block after the jump");
    }

    #[test]
    fn keylock_hand_over_is_seamless_at_every_rate() {
        let track = sine_track(440.0, 8.0);
        for tempo in [-1.0, -0.4, 0.0, 0.5, 1.0] {
            let mut p = Player::new(SR, 256);
            p.set_range(TempoRange::Sixteen);
            p.set_tempo(tempo);
            p.set_keylock(true);
            play(&mut p, &track, true, 0.5);
            p.jump_to(2.0 * SR as f64);
            let out = play(&mut p, &track, true, 0.3);
            for (n, block) in out.chunks(256).enumerate() {
                let peak = block.iter().fold(0.0f32, |m, v| m.max(v.abs()));
                assert!(peak > 0.3, "tempo {tempo}: block {n} peak {peak}");
            }
        }
    }

    /// Lag (in samples) that best aligns `out` with `reference`; positive = `out` is late.
    fn best_lag(out: &[f32], reference: &[f32], max_lag: i64) -> i64 {
        let mut best = (0i64, f32::MIN);
        for lag in -max_lag..=max_lag {
            let mut acc = 0.0f32;
            for (i, o) in out.iter().enumerate() {
                let j = i as i64 - lag;
                if j >= 0 && (j as usize) < reference.len() {
                    acc += o * reference[j as usize];
                }
            }
            if acc > best.1 {
                best = (lag, acc);
            }
        }
        best.0
    }

    #[test]
    fn keylock_output_lines_up_with_the_playhead() {
        // A click every 100 ms over a quiet tone (the tone keeps the stretcher out of its
        // silence mode).
        let n = 6 * SR as usize;
        let samples: Vec<f32> = (0..n)
            .flat_map(|i| {
                let click = if i % 4800 < 24 { 0.8 } else { 0.0 };
                let v = click + 0.05 * (i as f32 * 220.0 * std::f32::consts::TAU / SR as f32).sin();
                [v, v]
            })
            .collect();
        let track = TrackAudio::new(1, SR, samples);
        for tempo in [0.0f32, 1.0] {
            let mut p = Player::new(SR, 256);
            p.set_range(TempoRange::Ten);
            p.set_tempo(tempo);
            p.set_keylock(true);
            play(&mut p, &track, true, 1.0);
            // Record output together with what the playhead says should be heard.
            let mut out = vec![0.0; 256 * 2];
            let mut heard = Vec::new();
            let mut expected = Vec::new();
            for _ in 0..40 {
                let start = p.playhead();
                let rate = f64::from(p.tempo_rate());
                p.render(&track, true, &mut out);
                for (k, f) in out.as_chunks::<2>().0.iter().enumerate() {
                    heard.push(f[0]);
                    expected.push(sample_at(&track, start + k as f64 * rate)[0]);
                }
            }
            let lag = best_lag(&heard, &expected, 2400);
            assert!(
                lag.abs() <= 96,
                "tempo {tempo}: keylock output off by {lag} samples"
            );
        }
    }

    #[test]
    fn reverse_plays_backwards_and_stops_at_the_start() {
        let track = sine_track(440.0, 2.0);
        let mut p = Player::new(SR, 256);
        p.jump_to(0.5 * SR as f64);
        p.set_reverse(true);
        play(&mut p, &track, true, 0.25);
        assert!(
            (p.playhead() - 0.25 * SR as f64).abs() < 300.0,
            "{}",
            p.playhead()
        );
        let mut out = vec![0.0; 256 * 2];
        let mut hit = Boundaries::default();
        for _ in 0..200 {
            let h = p.render(&track, true, &mut out);
            hit.start |= h.start;
        }
        assert!(hit.start);
        assert_eq!(p.playhead(), 0.0);
    }

    #[test]
    fn holding_the_platter_stops_and_releasing_resumes() {
        let track = sine_track(440.0, 4.0);
        let mut p = Player::new(SR, 256);
        play(&mut p, &track, true, 0.2);
        p.scratch_touch(true);
        play(&mut p, &track, true, 0.1);
        assert!(p.rate().abs() < 0.01, "held platter: {}", p.rate());
        p.scratch_touch(false);
        play(&mut p, &track, true, 0.3);
        assert!((p.rate() - 1.0).abs() < 0.01, "back to speed: {}", p.rate());
    }

    #[test]
    fn scratching_follows_the_platter() {
        let track = sine_track(440.0, 4.0);
        let mut p = Player::new(SR, 256);
        p.jump_to(SR as f64);
        p.scratch_touch(true);
        // Turn the platter backwards at 33⅓ rpm for 0.2 s while the deck is paused.
        let ticks_per_block = -(JOG_TICKS_PER_REV * PLATTER_REV_PER_SEC * 256.0 / SR as f32);
        let mut out = vec![0.0; 256 * 2];
        let mut carry = 0.0f32;
        for _ in 0..(0.2 * SR as f32 / 256.0) as usize {
            carry += ticks_per_block;
            let whole = carry.trunc();
            carry -= whole;
            p.scratch(whole as i32);
            p.render(&track, false, &mut out);
        }
        assert!((p.rate() + 1.0).abs() < 0.1, "rate {}", p.rate());
        let moved = SR as f64 - p.playhead();
        assert!(
            (moved - 0.2 * SR as f64).abs() < 0.03 * SR as f64,
            "moved {moved}"
        );
    }

    #[test]
    fn bend_from_the_rim_fades_away() {
        let track = sine_track(440.0, 4.0);
        let mut p = Player::new(SR, 256);
        play(&mut p, &track, true, 0.1);
        let mut out = vec![0.0; 256 * 2];
        for _ in 0..10 {
            p.bend(5);
            p.render(&track, true, &mut out);
        }
        assert!(p.rate() > 1.05, "bent up: {}", p.rate());
        play(&mut p, &track, true, 0.5);
        assert!((p.rate() - 1.0).abs() < 0.005, "back: {}", p.rate());
    }

    #[test]
    fn bend_buttons_hold_a_fixed_offset() {
        let track = sine_track(440.0, 4.0);
        let mut p = Player::new(SR, 256);
        p.bend_hold(-1);
        play(&mut p, &track, true, 0.5);
        assert!((p.rate() - (1.0 - BEND_HOLD)).abs() < 0.002, "{}", p.rate());
    }

    #[test]
    fn search_jumps_and_paused_stays_silent() {
        let track = sine_track(440.0, 4.0);
        let mut p = Player::new(SR, 256);
        p.search(10, track.frames() as u64);
        assert!((p.playhead() - 0.5 * SR as f64).abs() < 1.0);
        let out = play(&mut p, &track, false, 0.1);
        assert!(out.iter().all(|v| *v == 0.0));
        assert!(
            (p.playhead() - 0.5 * SR as f64).abs() < 1.0,
            "paused does not move"
        );
    }

    #[test]
    fn playing_to_the_end_reports_it() {
        let track = sine_track(440.0, 0.1);
        let mut p = Player::new(SR, 256);
        let mut out = vec![0.0; 256 * 2];
        let mut end = false;
        for _ in 0..40 {
            end |= p.render(&track, true, &mut out).end;
        }
        assert!(end);
        assert_eq!(p.playhead(), track.frames() as f64);
    }
}
