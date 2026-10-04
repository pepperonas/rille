//! Transition effects for taking a deck out of the mix with one button.
//!
//! - **Echo-out:** the deck fades out within ~150 ms while its last moment feeds a feedback
//!   delay; the echoes ring on and decay. When they are gone the deck is paused.
//! - **Filter-out:** a high-pass sweeps from 20 Hz to 8 kHz over two seconds while the level
//!   fades at the end; then the deck is paused.
//!
//! Pressing the button again while an effect runs cancels it: the dry signal comes back and the
//! remaining tail decays without pausing the deck. All memory is reserved up front.

use rille_core::{DeckId, TransitionKind, TransitionSnapshot};

use super::filter::{Mode, Svf, SvfCoefficients};
use crate::smooth::Smoother;

/// Echo time (until the tempo is known, a musical default: an eighth note at 80 BPM).
pub const ECHO_MS: f32 = 375.0;
pub const ECHO_FEEDBACK: f32 = 0.55;
/// The dry signal fades with this time constant (≈ 150 ms to silence).
const DRY_FADE_MS: f32 = 40.0;
/// How long the deck keeps feeding the delay after the press.
const SEND_HOLD_MS: f32 = 300.0;
const SEND_FADE_MS: f32 = 15.0;
/// Echoes below this level count as gone.
const QUIET: f32 = 1e-4;
pub const FILTER_SWEEP_MS: f32 = 2000.0;
const FILTER_FADE_FROM_MS: f32 = 1700.0;
const FILTER_FROM_HZ: f32 = 20.0;
const FILTER_TO_HZ: f32 = 8000.0;
const WET_FADE_MS: f32 = 40.0;
/// Coefficients of the filter sweep are recomputed this often (frames).
const SWEEP_STEP: u32 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Running,
    /// Cancelled: dry is coming back, the tail decays.
    Releasing,
}

/// What the engine has to do after a block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Continue,
    /// The effect is over. `pause`: it ran to the end and the deck should stop.
    Finished {
        deck: DeckId,
        pause: bool,
    },
}

pub struct Transition {
    sample_rate: u32,
    kind: TransitionKind,
    phase: Phase,
    deck: DeckId,
    elapsed: u32,
    dry: Smoother,
    send: Smoother,
    wet: Smoother,
    delay: Vec<f32>,
    delay_frames: usize,
    write: usize,
    /// Loudest echo sample since the last full delay period.
    echo_peak: f32,
    echo_peak_prev: f32,
    since_period: usize,
    hp: [Svf; 2],
    hp_coeffs: SvfCoefficients,
    /// Level fade at the end of Filter-out; frozen when the effect is cancelled so the
    /// filtered signal never jumps (wet then crossfades back to dry).
    fade: f32,
}

fn ms(sample_rate: u32, ms: f32) -> u32 {
    (sample_rate as f32 * ms / 1000.0) as u32
}

impl Transition {
    pub fn new(sample_rate: u32) -> Transition {
        let delay_frames = ms(sample_rate, ECHO_MS).max(1) as usize;
        Transition {
            sample_rate,
            kind: TransitionKind::default(),
            phase: Phase::Idle,
            deck: DeckId::A,
            elapsed: 0,
            dry: Smoother::new(sample_rate, DRY_FADE_MS, 1.0),
            send: Smoother::new(sample_rate, SEND_FADE_MS, 0.0),
            wet: Smoother::new(sample_rate, WET_FADE_MS, 0.0),
            delay: vec![0.0; delay_frames * 2],
            delay_frames,
            write: 0,
            echo_peak: 0.0,
            echo_peak_prev: 0.0,
            since_period: 0,
            hp: [Svf::default(); 2],
            hp_coeffs: SvfCoefficients::new(FILTER_FROM_HZ, sample_rate),
            fade: 1.0,
        }
    }

    pub fn kind(&self) -> TransitionKind {
        self.kind
    }

    /// Change the effect; only while idle, a running effect keeps its kind.
    pub fn cycle(&mut self) {
        if self.phase == Phase::Idle {
            self.kind = self.kind.next();
        }
    }

    /// The deck the effect is on, if any.
    pub fn active(&self) -> Option<DeckId> {
        (self.phase != Phase::Idle).then_some(self.deck)
    }

    pub fn snapshot(&self) -> TransitionSnapshot {
        TransitionSnapshot {
            kind: self.kind,
            deck: self.active(),
            releasing: self.phase == Phase::Releasing,
        }
    }

    /// Button press: start on `deck` when idle, cancel when running.
    pub fn trigger(&mut self, deck: DeckId) {
        match self.phase {
            Phase::Idle => {
                self.phase = Phase::Running;
                self.deck = deck;
                self.elapsed = 0;
                self.dry.snap();
                self.dry.set_target(match self.kind {
                    TransitionKind::EchoOut => 0.0,
                    TransitionKind::FilterOut => 1.0,
                });
                self.send.set_target(1.0);
                self.send.snap();
                self.wet.set_target(1.0);
                self.wet.snap();
                self.hp = [Svf::default(); 2];
                self.fade = 1.0;
            }
            Phase::Running => {
                self.phase = Phase::Releasing;
                self.dry.set_target(1.0);
                self.send.set_target(0.0);
                self.wet.set_target(0.0);
            }
            Phase::Releasing => {}
        }
    }

    /// The deck got new material (track loaded, unloaded, started again): a running effect on
    /// it lets go — the dry signal comes back, the tail decays, and nothing gets paused.
    pub fn release_deck(&mut self, deck: DeckId) {
        if self.phase == Phase::Running && self.deck == deck {
            self.trigger(deck);
        }
    }

    /// Process one post-fader frame of the active deck; returns what that deck contributes.
    #[inline]
    pub fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        match self.kind {
            TransitionKind::EchoOut => self.echo(x),
            TransitionKind::FilterOut => self.filter(x),
        }
    }

    fn echo(&mut self, x: [f32; 2]) -> [f32; 2] {
        if self.phase == Phase::Running && self.elapsed == ms(self.sample_rate, SEND_HOLD_MS) {
            self.send.set_target(0.0);
        }
        let dry = self.dry.tick();
        let send = self.send.tick();
        let i = self.write * 2;
        let y = [self.delay[i], self.delay[i + 1]];
        for ch in 0..2 {
            let mut v = x[ch] * send + y[ch] * ECHO_FEEDBACK;
            if v.abs() < 1e-20 {
                v = 0.0;
            }
            self.delay[i + ch] = v;
        }
        self.write = (self.write + 1) % self.delay_frames;
        self.echo_peak = self.echo_peak.max(y[0].abs()).max(y[1].abs());
        self.since_period += 1;
        if self.since_period >= self.delay_frames {
            self.echo_peak_prev = self.echo_peak;
            self.echo_peak = 0.0;
            self.since_period = 0;
        }
        self.elapsed = self.elapsed.saturating_add(1);
        [x[0] * dry + y[0], x[1] * dry + y[1]]
    }

    fn filter(&mut self, x: [f32; 2]) -> [f32; 2] {
        let t = self.elapsed as f32 * 1000.0 / self.sample_rate as f32;
        if self.phase == Phase::Running && self.elapsed.is_multiple_of(SWEEP_STEP) {
            let progress = (t / FILTER_SWEEP_MS).clamp(0.0, 1.0);
            let cutoff = FILTER_FROM_HZ * (FILTER_TO_HZ / FILTER_FROM_HZ).powf(progress);
            self.hp_coeffs = SvfCoefficients::new(cutoff, self.sample_rate);
        }
        if self.phase == Phase::Running {
            self.fade = (1.0 - (t - FILTER_FADE_FROM_MS) / (FILTER_SWEEP_MS - FILTER_FADE_FROM_MS))
                .clamp(0.0, 1.0);
        }
        let fade = self.fade;
        let wet = self.wet.tick();
        let mut out = [0.0; 2];
        for ch in 0..2 {
            let filtered = self.hp[ch].process(x[ch], &self.hp_coeffs, Mode::HighPass) * fade;
            out[ch] = x[ch] + wet * (filtered - x[ch]);
        }
        self.elapsed = self.elapsed.saturating_add(1);
        out
    }

    /// Call after every block. Decides whether the effect is over.
    pub fn end_of_block(&mut self) -> Outcome {
        let finished = match (self.phase, self.kind) {
            (Phase::Idle, _) => return Outcome::Continue,
            (Phase::Running, TransitionKind::EchoOut) => {
                self.elapsed > ms(self.sample_rate, SEND_HOLD_MS + 2.0 * ECHO_MS)
                    && self.echo_quiet()
            }
            (Phase::Running, TransitionKind::FilterOut) => {
                self.elapsed >= ms(self.sample_rate, FILTER_SWEEP_MS)
            }
            (Phase::Releasing, TransitionKind::EchoOut) => {
                (self.dry.current() - 1.0).abs() < 1e-3 && self.echo_quiet()
            }
            (Phase::Releasing, TransitionKind::FilterOut) => self.wet.current() < 1e-3,
        };
        if !finished {
            return Outcome::Continue;
        }
        let pause = self.phase == Phase::Running;
        self.reset();
        Outcome::Finished {
            deck: self.deck,
            pause,
        }
    }

    fn echo_quiet(&self) -> bool {
        self.echo_peak_prev < QUIET && self.echo_peak < QUIET && self.send.current() < 1e-3
    }

    fn reset(&mut self) {
        self.phase = Phase::Idle;
        self.dry.set_target(1.0);
        self.dry.snap();
        self.send.set_target(0.0);
        self.send.snap();
        self.wet.set_target(0.0);
        self.wet.snap();
        self.delay.fill(0.0);
        self.echo_peak = 0.0;
        self.echo_peak_prev = 0.0;
        self.since_period = 0;
        self.hp = [Svf::default(); 2];
        self.fade = 1.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 48_000;

    /// Run `seconds` of a constant signal through the effect, return the outputs (left) and the
    /// first non-continue outcome with its time.
    fn run(t: &mut Transition, seconds: f32, input: f32) -> (Vec<f32>, Option<(f32, Outcome)>) {
        run_with(t, seconds, |_| input)
    }

    fn run_with(
        t: &mut Transition,
        seconds: f32,
        input: impl Fn(usize) -> f32,
    ) -> (Vec<f32>, Option<(f32, Outcome)>) {
        let mut out = Vec::new();
        let mut outcome = None;
        let blocks = (seconds * SR as f32 / 256.0) as usize;
        for b in 0..blocks {
            for _ in 0..256 {
                let x = input(out.len());
                out.push(t.process([x, x])[0]);
            }
            if outcome.is_none() {
                let o = t.end_of_block();
                if o != Outcome::Continue {
                    outcome = Some(((b + 1) as f32 * 256.0 / SR as f32, o));
                }
            }
        }
        (out, outcome)
    }

    fn at(v: &[f32], seconds: f32) -> f32 {
        v[(seconds * SR as f32) as usize]
    }

    #[test]
    fn echo_out_fades_the_deck_and_leaves_echoes() {
        let mut t = Transition::new(SR);
        t.trigger(DeckId::B);
        assert_eq!(t.active(), Some(DeckId::B));
        // Alternating input so the dry part and the echo are distinguishable is overkill: with
        // a constant input the output first drops (dry fades) and later shows echo bumps.
        let (out, outcome) = run(&mut t, 12.0, 0.5);
        assert!(at(&out, 0.0) > 0.45, "starts at full level");
        let echoes = out[(0.4 * SR as f32) as usize..(1.5 * SR as f32) as usize]
            .iter()
            .fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(echoes > 0.1, "echoes keep sounding after the fade");
        let (time, outcome) = outcome.expect("finishes");
        assert_eq!(
            outcome,
            Outcome::Finished {
                deck: DeckId::B,
                pause: true
            }
        );
        assert!(time < 10.0, "ends after the echoes died ({time:.1} s)");
        assert_eq!(t.active(), None);
    }

    #[test]
    fn echo_out_dry_is_gone_quickly() {
        // Feed silence after the hold so only the dry path could contribute.
        let mut t = Transition::new(SR);
        t.trigger(DeckId::A);
        let mut last = 0.0;
        for i in 0..(0.2 * SR as f32) as usize {
            // Impulse train far apart from the echo time: dry path only at i == 0.
            last = t.process(if i == 0 { [1.0, 1.0] } else { [0.0, 0.0] })[0];
        }
        assert!(last.abs() < 1e-3);
        // A constant input at 200 ms: dry gain must be tiny by now.
        let mut t = Transition::new(SR);
        t.trigger(DeckId::A);
        for _ in 0..(0.2 * SR as f32) as usize {
            t.process([0.0, 0.0]);
        }
        assert!(t.dry.current() < 0.01, "dry {}", t.dry.current());
    }

    #[test]
    fn cancel_brings_the_deck_back_without_pausing() {
        let mut t = Transition::new(SR);
        t.trigger(DeckId::A);
        run(&mut t, 0.2, 0.5);
        t.trigger(DeckId::A); // cancel
        assert!(t.snapshot().releasing);
        let (out, outcome) = run(&mut t, 8.0, 0.5);
        assert!(
            out.last().is_some_and(|v| (v - 0.5).abs() < 1e-3),
            "dry back"
        );
        assert_eq!(
            outcome.map(|o| o.1),
            Some(Outcome::Finished {
                deck: DeckId::A,
                pause: false
            })
        );
    }

    #[test]
    fn filter_out_removes_lows_and_pauses_after_the_sweep() {
        let mut t = Transition::new(SR);
        t.cycle();
        assert_eq!(t.kind(), TransitionKind::FilterOut);
        t.trigger(DeckId::A);
        let sine = |i: usize| 0.5 * (i as f32 * 200.0 * std::f32::consts::TAU / SR as f32).sin();
        let (out, outcome) = run_with(&mut t, 3.0, sine);
        let peak = |from: f32, to: f32| {
            out[(from * SR as f32) as usize..(to * SR as f32) as usize]
                .iter()
                .fold(0.0f32, |m, v| m.max(v.abs()))
        };
        assert!(
            peak(0.05, 0.15) > 0.45,
            "200 Hz passes at first (20 Hz high-pass)"
        );
        assert!(
            peak(1.45, 1.55) < 0.05,
            "and is gone once the cutoff is past 1.5 kHz"
        );
        let (time, outcome) = outcome.expect("finishes");
        assert_eq!(
            outcome,
            Outcome::Finished {
                deck: DeckId::A,
                pause: true
            }
        );
        assert!((time - 2.0).abs() < 0.05);
    }

    #[test]
    fn cancelling_filter_out_late_does_not_jump_in_level() {
        let mut t = Transition::new(SR);
        t.cycle();
        t.trigger(DeckId::A);
        // Broadband input so the high-passed signal stays strong.
        let noise = |i: usize| {
            if (i.wrapping_mul(2_654_435_761) >> 7).is_multiple_of(2) {
                0.5
            } else {
                -0.5
            }
        };
        let (before, _) = run_with(&mut t, 1.9, noise);
        t.trigger(DeckId::A); // cancel inside the end fade (fade ≈ 0.33)
        let n = before.len();
        let after: Vec<f32> = (0..64)
            .map(|i| t.process([noise(n + i), noise(n + i)])[0])
            .collect();
        let rms = |v: &[f32]| (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt();
        let ratio = rms(&after) / rms(&before[n - 64..]);
        assert!(ratio < 1.5, "level jumped by {ratio:.2}x on cancel");
    }

    #[test]
    fn new_material_releases_the_effect_without_pausing() {
        let mut t = Transition::new(SR);
        t.trigger(DeckId::B);
        run(&mut t, 0.3, 0.5);
        t.release_deck(DeckId::A); // other deck: no effect
        assert!(!t.snapshot().releasing);
        t.release_deck(DeckId::B);
        assert!(t.snapshot().releasing);
        let (_, outcome) = run(&mut t, 10.0, 0.5);
        assert_eq!(
            outcome.map(|o| o.1),
            Some(Outcome::Finished {
                deck: DeckId::B,
                pause: false
            })
        );
    }

    #[test]
    fn kind_only_changes_while_idle() {
        let mut t = Transition::new(SR);
        t.trigger(DeckId::A);
        t.cycle();
        assert_eq!(t.kind(), TransitionKind::EchoOut);
    }

    #[test]
    fn idle_effect_is_transparent_and_silent_afterwards() {
        let mut t = Transition::new(SR);
        assert_eq!(t.end_of_block(), Outcome::Continue);
        t.trigger(DeckId::A);
        run(&mut t, 12.0, 0.0);
        assert!(t.delay.iter().all(|v| *v == 0.0), "no denormal leftovers");
    }
}
