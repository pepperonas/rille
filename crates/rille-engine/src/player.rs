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
/// Platter speed is measured over this much recent time. Long enough to hold several ticks
/// even on a slow drag with small buffers, short enough to follow a scratch back and forth.
const SCRATCH_WINDOW_S: f32 = 0.02;
/// Blocks remembered for the speed window (64-frame blocks need 15 for 20 ms at 48 kHz).
const SCRATCH_HISTORY: usize = 32;
/// Seconds moved per search tick (shift + platter).
pub const SEARCH_SECONDS_PER_TICK: f64 = 0.05;
/// Highest playback rate the keylock input buffer is sized for (wide range plus bend).
const MAX_KEYLOCK_RATE: f32 = 2.25;
const TEMPO_SMOOTHING_MS: f32 = 30.0;
const SCRATCH_SMOOTHING_MS: f32 = 6.0;
const BEND_SMOOTHING_MS: f32 = 60.0;
/// Frames over which the varispeed bridge hands over to the keylock stretcher.
const KEYLOCK_CROSSFADE: usize = 256;
/// Fade-out of the stretcher when keylock stops (pause, jump, reverse, scratch, KEY off), the
/// same length as the engine's declick.
const KEYLOCK_FADE_MS: f32 = 4.0;

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
    /// Recent blocks while scratching: (ticks, frames), newest at `scratch_head`.
    scratch_hist: [(i32, u32); SCRATCH_HISTORY],
    scratch_head: usize,
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
    /// While > 0 the stretcher keeps running from where it was and fades out over the new
    /// output: its audio is not sample-identical to the raw track, so a hard switch clicks.
    fade_left: usize,
    fade_len: usize,
    fade_feed: f64,
    fade_rate: f32,
}

#[inline]
fn hermite(xm1: f32, x0: f32, x1: f32, x2: f32, t: f32) -> f32 {
    let c1 = 0.5 * (x1 - xm1);
    let c2 = xm1 - 2.5 * x0 + 2.0 * x1 - 0.5 * x2;
    let c3 = 0.5 * (x2 - xm1) + 1.5 * (x0 - x1);
    ((c3 * t + c2) * t + c1) * t + x0
}

/// Copy track frames from `first` on into `buf` (interleaved stereo); before the start is silence.
fn fill_input(track: &TrackAudio, first: i64, buf: &mut [f32]) {
    for (k, frame) in buf.as_chunks_mut::<2>().0.iter_mut().enumerate() {
        let i = first + k as i64;
        *frame = if i < 0 {
            [0.0; 2]
        } else {
            track.frame(i as usize)
        };
    }
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
        let fade_len = ((sample_rate as f32 * KEYLOCK_FADE_MS / 1000.0) as usize).max(1);
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
            scratch_hist: [(0, 0); SCRATCH_HISTORY],
            scratch_head: 0,
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
            fade_left: 0,
            fade_len,
            fade_feed: 0.0,
            fade_rate: 0.0,
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

    /// Keylock output is sounding or still fading out. The engine's declick then leaves the
    /// fade-out to the player: a tail read from the raw track would not match what was heard.
    pub fn owns_fade_out(&self) -> bool {
        self.keylock_engaged || self.fade_left > 0
    }

    /// Leave keylock: let the stretcher fade out from where it is instead of cutting it.
    fn leave_keylock(&mut self) {
        if self.keylock_engaged {
            self.keylock_engaged = false;
            self.fade_left = self.fade_len;
            self.fade_feed = self.feed;
            self.fade_rate = self.last_rate.clamp(0.0, MAX_KEYLOCK_RATE);
        }
    }

    /// New track or a jump (cue, seek): continue from `position`.
    pub fn jump_to(&mut self, position: f64) {
        self.playhead = position.max(0.0);
        self.leave_keylock();
    }

    /// Fresh state for a newly loaded track (tempo, range and keylock stay, like on a CDJ).
    pub fn reset_for_track(&mut self) {
        self.jump_to(0.0);
        // The old track is gone: nothing left to fade out.
        self.fade_left = 0;
        // A load during a scratch: back to tempo smoothing, as a release would do.
        self.rate.set_time(self.sample_rate, TEMPO_SMOOTHING_MS);
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
        if !on {
            self.leave_keylock();
        }
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
        self.scratch_hist = [(0, 0); SCRATCH_HISTORY];
        self.leave_keylock();
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
        self.leave_keylock();
    }

    fn update_targets(&mut self, playing: bool, frames: usize) {
        let dt = frames as f32 / self.sample_rate as f32;
        if self.scratching {
            // Speed = ticks over the last ~20 ms, not over one block: on a slow drag with small
            // buffers most blocks carry no tick, and a per-block estimate jumps between
            // standstill and several times the real speed. No ticks in the window = held still.
            let _ = dt;
            self.scratch_head = (self.scratch_head + 1) % SCRATCH_HISTORY;
            self.scratch_hist[self.scratch_head] = (self.scratch_ticks, frames as u32);
            self.scratch_ticks = 0;
            let window = (SCRATCH_WINDOW_S * self.sample_rate as f32) as u32;
            let (mut ticks, mut span) = (0i32, 0u32);
            for back in 0..SCRATCH_HISTORY {
                let (t, f) = self.scratch_hist
                    [(self.scratch_head + SCRATCH_HISTORY - back) % SCRATCH_HISTORY];
                ticks = ticks.saturating_add(t);
                span = span.saturating_add(f);
                if span >= window || f == 0 {
                    break;
                }
            }
            let seconds = span.max(1) as f32 / self.sample_rate as f32;
            self.rate
                .set_target(ticks as f32 / seconds / (JOG_TICKS_PER_REV * PLATTER_REV_PER_SEC));
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
        let wants_keylock = self.keylock
            && playing
            && !self.scratching
            && !self.reverse
            && self.rate.target() > 0.0;
        if !wants_keylock {
            self.leave_keylock();
        }
        // A running fade-out occupies the stretcher; keylock re-engages right after it, and only
        // once the rate has settled (after reverse or a scratch it may still glide from far off).
        // Fed while gliding from near zero, the stretcher works with a stretch factor above 2,
        // where Signalsmith randomises phases (seeded from the OS) and the alignment it settles
        // into becomes a matter of chance. Until then varispeed plays, as during the bridge.
        let settled = self.keylock_engaged || self.rate_settled();
        if wants_keylock && self.fade_left == 0 && settled {
            self.render_keylock(track, out)
        } else {
            let hit = self.render_varispeed(track, out);
            if self.fade_left > 0 {
                self.mix_fade_out(track, out);
            }
            hit
        }
    }

    /// The rate is within 2 % of where it is heading.
    fn rate_settled(&self) -> bool {
        let target = self.rate.target();
        (self.rate.current() - target).abs() <= 0.02 * target.abs().max(0.1)
    }

    /// Crossfade from the stretcher (continuing at its last rate) into `out`.
    fn mix_fade_out(&mut self, track: &TrackAudio, out: &mut [f32]) {
        let frames = out.len() / 2;
        let advance = f64::from(self.fade_rate) * frames as f64;
        let first = self.fade_feed.floor() as i64;
        let wanted = ((self.fade_feed + advance).floor() as i64 - first).max(0) as usize;
        let n_in = wanted.min(self.stretch_in.len() / 2);
        fill_input(track, first, &mut self.stretch_in[..n_in * 2]);
        let stretched = &mut self.stretch_out[..frames * 2];
        self.stretch
            .process(&self.stretch_in[..n_in * 2], &mut *stretched);
        let len = self.fade_len as f32;
        for (k, (o, s)) in out
            .as_chunks_mut::<2>()
            .0
            .iter_mut()
            .zip(stretched.as_chunks::<2>().0)
            .enumerate()
        {
            let g = self.fade_left.saturating_sub(k) as f32 / len;
            o[0] += g * (s[0] - o[0]);
            o[1] += g * (s[1] - o[1]);
        }
        self.fade_feed += advance;
        self.fade_left = self.fade_left.saturating_sub(frames);
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
        fill_input(track, start, &mut self.preroll);
        self.stretch.seek(&self.preroll, f64::from(rate));
        self.keylock_engaged = true;
        self.since_engage = 0;
        self.bridge_pos = self.playhead;
    }

    fn render_keylock(&mut self, track: &TrackAudio, out: &mut [f32]) -> Boundaries {
        let frames = out.len() / 2;
        if !self.keylock_engaged {
            // The target, not the gliding value: after reverse or a scratch the smoother may
            // still be far off, and the feed offset must match the rate the stretcher settles at.
            let rate =
                (self.rate.target() * (1.0 + self.bend.target())).clamp(0.0, MAX_KEYLOCK_RATE);
            self.engage_keylock(track, rate);
        }
        let bridge_until = self.stretch.output_latency() + KEYLOCK_CROSSFADE;
        let bridging = self.since_engage < bridge_until;
        // Per-sample rates: the stretcher takes their sum, the bridge uses them directly.
        let mut advance = 0.0f64;
        let mut rate = 0.0;
        for frame in out.as_chunks_mut::<2>().0 {
            // Not clamped: right after a scratch release the smoother can be far above
            // MAX_KEYLOCK_RATE. The stretcher input is then capped (`n_in` below) and loses
            // frames, but that happens inside the varispeed bridge, which is what is heard, and
            // feed and playhead still advance together, so alignment holds (tested). Clamping
            // here would instead make the record stop decelerating and drop to 2.25× at once.
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
        fill_input(track, first, &mut self.stretch_in[..n_in * 2]);
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

    /// Clicks at irregular intervals (50–150 ms) over a quiet tone, for alignment
    /// measurements. Irregular, so a correlation cannot lock onto a neighbouring click.
    fn click_track() -> TrackAudio {
        let n = 8 * SR as usize;
        let mut clicks = vec![false; n];
        let (mut at, mut seed) = (0usize, 12345u32);
        while at < n {
            clicks[at..(at + 24).min(n)].fill(true);
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            at += 2400 + (seed >> 16) as usize % 4800;
        }
        let samples: Vec<f32> = (0..n)
            .flat_map(|i| {
                let click = if clicks[i] { 0.8 } else { 0.0 };
                let v = click + 0.05 * (i as f32 * 220.0 * std::f32::consts::TAU / SR as f32).sin();
                [v, v]
            })
            .collect();
        TrackAudio::new(1, SR, samples)
    }

    /// Play on, after the bridge has handed over, and measure how far keylock output lags the
    /// playhead.
    fn lag_after(p: &mut Player, track: &TrackAudio) -> i64 {
        play(p, track, true, 0.5);
        let mut out = vec![0.0; 256 * 2];
        let mut heard = Vec::new();
        let mut expected = Vec::new();
        // 1.5 s holds 10–30 irregular clicks; ±2400 still covers the 1380-sample error the
        // reverse case had before the fix.
        for _ in 0..280 {
            let start = p.playhead();
            let rate = f64::from(p.tempo_rate());
            p.render(track, true, &mut out);
            for (k, f) in out.as_chunks::<2>().0.iter().enumerate() {
                heard.push(f[0]);
                expected.push(sample_at(track, start + k as f64 * rate)[0]);
            }
        }
        best_lag(&heard, &expected, 2400)
    }

    #[test]
    fn steady_scratch_gives_a_steady_rate_even_with_small_blocks() {
        let track = sine_track(440.0, 30.0);
        // A slow drag: at 200 ticks/s most 64-frame blocks carry no tick at all.
        for (block, tps) in [(64usize, 1000.0), (256, 1000.0), (64, 200.0), (256, 200.0)] {
            let mut p = Player::new(SR, block);
            p.scratch_touch(true);
            let mut out = vec![0.0; block * 2];
            // Ticks arrive as whole ticks per block, like MIDI messages.
            let per_block = tps * block as f64 / f64::from(SR);
            let mut owed = 0.0;
            let mut rates = Vec::new();
            for n in 0..(SR as usize / block) {
                owed += per_block;
                let ticks = owed.floor();
                owed -= ticks;
                p.scratch(ticks as i32);
                p.render(&track, false, &mut out);
                if n * block > SR as usize / 4 {
                    rates.push(f64::from(p.rate()));
                }
            }
            let expected = tps / f64::from(JOG_TICKS_PER_REV * PLATTER_REV_PER_SEC);
            let worst = rates
                .iter()
                .map(|r| (r - expected).abs() / expected)
                .fold(0.0, f64::max);
            assert!(
                worst < 0.2,
                "block {block}, {tps} ticks/s: rate off by up to {:.0} %",
                worst * 100.0
            );
        }
    }

    #[test]
    fn keylock_stays_aligned_after_reverse_release() {
        let track = click_track();
        let mut p = Player::new(SR, 256);
        p.set_range(TempoRange::Ten);
        p.set_tempo(1.0);
        p.set_keylock(true);
        play(&mut p, &track, true, 1.5);
        p.set_reverse(true);
        play(&mut p, &track, true, 0.3);
        p.set_reverse(false);
        let lag = lag_after(&mut p, &track);
        assert!(
            lag.abs() <= 96,
            "keylock off by {lag} samples after reverse"
        );
    }

    #[test]
    fn keylock_stays_aligned_after_a_fast_scratch() {
        let track = click_track();
        let mut p = Player::new(SR, 256);
        p.set_keylock(true);
        play(&mut p, &track, true, 1.5);
        p.scratch_touch(true);
        let mut out = vec![0.0; 256 * 2];
        for _ in 0..20 {
            // Far faster than the keylock input buffer is sized for.
            p.scratch(100);
            p.render(&track, true, &mut out);
        }
        p.scratch_touch(false);
        let lag = lag_after(&mut p, &track);
        assert!(
            lag.abs() <= 96,
            "keylock off by {lag} samples after scratching"
        );
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
