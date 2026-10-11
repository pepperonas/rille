#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code

//! Sync, sync lock, beat loops, beat jumps and hot cues, through the engine's public API.

use std::sync::Arc;

use rille_core::{
    BeatGrid, Command, DeckCommand, DeckId, HOT_CUES, Snapshot, TempoRange, TrackAudio,
};
use rille_engine::{Engine, EngineHandle, TrackLoad, engine_pair};

const SR: u32 = 48_000;
const BLOCK: usize = 256;

/// A click on every beat (so loops and phase are audible in the data) over a quiet tone.
fn beat_track(id: u64, grid: BeatGrid, seconds: f64) -> Arc<TrackAudio> {
    let frames = (seconds * f64::from(SR)) as usize;
    let period = 60.0 / grid.bpm * f64::from(SR);
    let first = grid.first_beat * f64::from(SR);
    let samples = (0..frames)
        .flat_map(|i| {
            let since = (i as f64 - first).rem_euclid(period);
            let v = 0.1 * (i as f32 * 0.031).sin() + if since < 200.0 { 0.6 } else { 0.0 };
            [v, v]
        })
        .collect();
    Arc::new(TrackAudio::new(id, SR, samples))
}

struct Rig {
    engine: Engine,
    handle: EngineHandle,
    out: Vec<f32>,
}

impl Rig {
    fn new(a: BeatGrid, b: BeatGrid) -> Rig {
        let (engine, mut handle) = engine_pair(SR, BLOCK);
        for (deck, grid, id) in [(DeckId::A, a, 1), (DeckId::B, b, 2)] {
            let load = TrackLoad {
                grid: Some(grid),
                ..TrackLoad::new(deck, beat_track(id, grid, 120.0))
            };
            assert!(handle.load_track(load).is_ok());
        }
        let mut rig = Rig {
            engine,
            handle,
            out: vec![0.0; BLOCK * 2],
        };
        rig.run(0.01);
        rig
    }

    fn send(&mut self, deck: DeckId, cmd: DeckCommand) {
        self.handle.send(Command::Deck(deck, cmd));
    }

    fn run(&mut self, seconds: f64) {
        let blocks = (seconds * f64::from(SR) / BLOCK as f64).ceil() as usize;
        for _ in 0..blocks.max(1) {
            self.engine.process(&mut self.out, 2);
        }
    }

    fn snap(&mut self) -> Snapshot {
        self.handle.snapshot()
    }

    fn seek(&mut self, deck: DeckId, seconds: f64) {
        self.send(
            deck,
            DeckCommand::Seek {
                frame: (seconds * f64::from(SR)) as u64,
            },
        );
    }

    /// Beat position (fraction kept) of a deck.
    fn beat(&mut self, deck: DeckId) -> f64 {
        let d = self.snap().decks[deck.index()];
        let g = d.grid.unwrap();
        (d.position as f64 / f64::from(SR) - g.first_beat) * g.bpm / 60.0
    }
}

fn grid(bpm: f64, first_beat: f64) -> BeatGrid {
    BeatGrid { bpm, first_beat }
}

fn wrap(x: f64) -> f64 {
    x - (x + 0.5).floor()
}

#[test]
fn sync_matches_tempo_and_phase_of_the_playing_deck() {
    let mut rig = Rig::new(grid(120.0, 0.0), grid(124.0, 0.1));
    rig.seek(DeckId::A, 10.3);
    rig.seek(DeckId::B, 20.77);
    rig.send(DeckId::A, DeckCommand::Play);
    rig.send(DeckId::B, DeckCommand::Play);
    rig.run(0.5);
    rig.send(DeckId::B, DeckCommand::Sync);
    rig.run(1.0);
    let b = rig.snap().decks[1];
    let rate = TempoRange::Ten.rate(b.tempo);
    assert!(
        (f64::from(rate) * 124.0 - 120.0).abs() < 0.01,
        "B plays at {}",
        rate * 124.0
    );
    let error = wrap(rig.beat(DeckId::B) - rig.beat(DeckId::A));
    // One block of position granularity is 5 ms = 0.01 beat at 120 BPM.
    assert!(error.abs() < 0.015, "phase error {error} beats");
    // A one-shot sync does not lock: A's tempo change is not followed.
    rig.send(DeckId::A, DeckCommand::Tempo(0.5));
    rig.run(0.5);
    assert_eq!(rig.snap().decks[1].tempo, b.tempo);
}

#[test]
fn sync_follows_at_double_tempo_and_widens_the_range_when_needed() {
    let mut rig = Rig::new(grid(87.0, 0.0), grid(174.0, 0.0));
    rig.send(DeckId::A, DeckCommand::Play);
    rig.send(DeckId::B, DeckCommand::Sync);
    rig.run(0.1);
    let b = rig.snap().decks[1];
    assert!(
        b.tempo.abs() < 1e-4,
        "174 follows 87 at its own speed: {}",
        b.tempo
    );

    let mut rig = Rig::new(grid(128.0, 0.0), grid(112.0, 0.0));
    rig.send(DeckId::B, DeckCommand::Sync);
    rig.run(0.1);
    let b = rig.snap().decks[1];
    assert_eq!(
        b.tempo_range,
        TempoRange::Sixteen,
        "+14.3 % needs ±16 %: {b:?}"
    );
    assert!((b.tempo - (128.0 / 112.0 - 1.0) / 0.16).abs() < 1e-4);
}

#[test]
fn sync_lock_follows_tempo_changes_and_holds_the_phase() {
    let mut rig = Rig::new(grid(126.0, 0.05), grid(121.0, 0.21));
    rig.seek(DeckId::B, 5.0);
    rig.send(DeckId::A, DeckCommand::Play);
    rig.send(DeckId::B, DeckCommand::Play);
    rig.send(DeckId::B, DeckCommand::SyncLock(true));
    rig.run(1.0);
    assert!(rig.snap().decks[1].sync_lock);
    // The leader speeds up; the follower comes along and stays on the beat for 30 s.
    rig.send(DeckId::A, DeckCommand::Tempo(0.3));
    let mut worst = 0.0f64;
    for _ in 0..60 {
        rig.run(0.5);
        let e = wrap(rig.beat(DeckId::B) - rig.beat(DeckId::A));
        worst = worst.max(e.abs());
    }
    let s = rig.snap();
    let bpm = |i: usize, g: f64| g * f64::from(s.decks[i].tempo_range.rate(s.decks[i].tempo));
    assert!((bpm(1, 121.0) - bpm(0, 126.0)).abs() < 0.01);
    assert!(worst < 0.03, "phase drifted up to {worst} beats");
}

#[test]
fn sync_lock_pulls_a_phase_error_in_without_jumping() {
    let mut rig = Rig::new(grid(120.0, 0.0), grid(120.0, 0.0));
    rig.send(DeckId::A, DeckCommand::Play);
    rig.send(DeckId::B, DeckCommand::Play);
    rig.send(DeckId::B, DeckCommand::SyncLock(true));
    rig.run(0.5);
    // Knock B a fifth of a beat off by bending it, then let go.
    rig.send(DeckId::B, DeckCommand::BendHold(1));
    rig.run(2.5);
    rig.send(DeckId::B, DeckCommand::BendHold(0));
    rig.run(0.2);
    let off = wrap(rig.beat(DeckId::B) - rig.beat(DeckId::A));
    assert!(off.abs() > 0.1, "the bend did not move the phase: {off}");
    let mut last = rig.snap().decks[1].position;
    // At most 2 % faster or slower (inaudible as pitch): 0.1 s of error takes several seconds.
    for _ in 0..160 {
        rig.run(0.05);
        let pos = rig.snap().decks[1].position;
        // No jumps: every step is a normal stretch of playback (≤ 2.5 % faster).
        let step = pos - last;
        assert!(
            (step as f64) < 0.05 * f64::from(SR) * 1.025 + BLOCK as f64,
            "jumped {step}"
        );
        last = pos;
    }
    let e = wrap(rig.beat(DeckId::B) - rig.beat(DeckId::A));
    assert!(e.abs() < 0.01, "still {e} beats off after 8 s");
}

#[test]
fn beat_loop_repeats_exactly_and_can_be_left() {
    let g = grid(120.0, 0.0);
    let mut rig = Rig::new(g, g);
    rig.seek(DeckId::A, 10.1);
    rig.send(DeckId::A, DeckCommand::Play);
    rig.send(DeckId::A, DeckCommand::BeatLoop(4.0));
    rig.run(0.01);
    let (start, end) = rig.snap().decks[0].loop_range.unwrap();
    assert_eq!(
        start,
        10 * u64::from(SR),
        "loop starts on the beat at or before the playhead"
    );
    assert_eq!(end - start, 2 * u64::from(SR), "4 beats at 120 BPM");
    // Ten loops: the playhead never leaves.
    for _ in 0..200 {
        rig.run(0.1);
        let p = rig.snap().decks[0].position;
        assert!((start..end).contains(&p), "left the loop at {p}");
    }
    // Same length again: out of the loop, playing on.
    rig.send(DeckId::A, DeckCommand::BeatLoop(4.0));
    rig.run(3.0);
    let d = rig.snap().decks[0];
    assert_eq!(d.loop_range, None);
    assert!(d.position > end);
}

#[test]
fn the_loop_seam_lands_on_the_exact_frame() {
    // Render with the loop and compare the audio of two passes: identical after the 4 ms
    // crossfade, so the loop length is exact (no drift, no dropped frame).
    let g = grid(120.0, 0.0);
    let (mut engine, mut handle) = engine_pair(SR, BLOCK);
    let load = TrackLoad {
        grid: Some(g),
        ..TrackLoad::new(DeckId::A, beat_track(1, g, 30.0))
    };
    assert!(handle.load_track(load).is_ok());
    handle.send(Command::Deck(
        DeckId::A,
        DeckCommand::Seek { frame: 5 * 48_000 },
    ));
    handle.send(Command::Deck(DeckId::A, DeckCommand::Play));
    handle.send(Command::Deck(DeckId::A, DeckCommand::BeatLoop(1.0)));
    let mut audio = Vec::new();
    let mut out = vec![0.0f32; BLOCK * 2];
    for _ in 0..(5 * SR as usize / BLOCK) {
        engine.process(&mut out, 2);
        audio.extend(out.as_chunks::<2>().0.iter().map(|f| f[0]));
    }
    let period = 24_000; // one beat at 120 BPM
    let skip = 600; // past the crossfade at the seam
    // Pass 0 starts from silence, every later pass from the loop end: the EQ's filters (IIR,
    // with memory) only see the same input from pass 1 on, so passes 1, 2, 3 are compared.
    for pass in 2..4 {
        for k in skip..period - 200 {
            let (a, b) = (audio[k + period], audio[k + pass * period]);
            assert!((a - b).abs() < 1e-6, "pass {pass}, frame {k}: {a} vs {b}");
        }
    }
}

#[test]
fn a_loop_that_would_reach_the_track_end_is_not_made() {
    // The track would end instead of looping: no loop, playing on (an outro loop fits earlier).
    let g = grid(120.0, 0.0);
    let mut rig = Rig::new(g, g);
    rig.seek(DeckId::A, 119.0);
    rig.send(DeckId::A, DeckCommand::BeatLoop(8.0));
    rig.run(0.01);
    assert_eq!(rig.snap().decks[0].loop_range, None);
    rig.seek(DeckId::A, 110.0);
    rig.send(DeckId::A, DeckCommand::BeatLoop(8.0));
    rig.run(0.01);
    let sr = u64::from(SR);
    assert_eq!(rig.snap().decks[0].loop_range, Some((110 * sr, 114 * sr)));
    // Doubling up to 32 beats would pass the end: the loop stays as it is.
    rig.send(DeckId::A, DeckCommand::BeatLoop(32.0));
    rig.run(0.01);
    assert_eq!(rig.snap().decks[0].loop_range, Some((110 * sr, 114 * sr)));
}

#[test]
fn jumping_out_of_a_loop_leaves_it_and_lands_where_asked() {
    let g = grid(120.0, 0.0);
    let mut rig = Rig::new(g, g);
    let sr = u64::from(SR);
    rig.seek(DeckId::A, 60.0);
    rig.send(DeckId::A, DeckCommand::HotCue(0)); // cue at 1:00
    rig.seek(DeckId::A, 20.0);
    rig.send(DeckId::A, DeckCommand::Play);
    rig.send(DeckId::A, DeckCommand::BeatLoop(2.0));
    rig.run(0.5);
    assert!(rig.snap().decks[0].loop_range.is_some());
    rig.send(DeckId::A, DeckCommand::HotCue(0));
    rig.run(0.2);
    let d = rig.snap().decks[0];
    assert_eq!(d.loop_range, None, "the hot cue jumped out of the loop");
    assert!(
        d.position >= 60 * sr && d.position < 61 * sr,
        "{}",
        d.position
    );
    // A jump within the loop keeps it.
    rig.send(DeckId::A, DeckCommand::BeatLoop(4.0));
    rig.run(0.01);
    let (start, _) = rig.snap().decks[0].loop_range.unwrap();
    rig.send(
        DeckId::A,
        DeckCommand::Seek {
            frame: start + 1_000,
        },
    );
    rig.run(0.01);
    assert!(rig.snap().decks[0].loop_range.is_some());
}

#[test]
fn a_loop_press_just_before_the_beat_means_that_beat() {
    let g = grid(120.0, 0.0);
    let mut rig = Rig::new(g, g);
    let sr = u64::from(SR);
    rig.send(
        DeckId::A,
        DeckCommand::Seek {
            frame: 10 * sr - 1_000,
        },
    ); // 21 ms early
    rig.send(DeckId::A, DeckCommand::BeatLoop(4.0));
    rig.run(0.01);
    assert_eq!(rig.snap().decks[0].loop_range, Some((10 * sr, 12 * sr)));
    rig.send(DeckId::A, DeckCommand::LoopExit);
    rig.send(
        DeckId::A,
        DeckCommand::Seek {
            frame: 10 * sr - 6_000,
        },
    ); // a quarter beat early
    rig.send(DeckId::A, DeckCommand::BeatLoop(4.0));
    rig.run(0.01);
    assert_eq!(
        rig.snap().decks[0].loop_range,
        Some((9 * sr + sr / 2, 11 * sr + sr / 2))
    );
}

#[test]
fn a_beat_jump_that_would_push_the_loop_off_the_track_is_not_made() {
    let g = grid(120.0, 0.0);
    let mut rig = Rig::new(g, g);
    let sr = u64::from(SR);
    rig.seek(DeckId::A, 3.0);
    rig.send(DeckId::A, DeckCommand::BeatLoop(4.0));
    rig.send(DeckId::A, DeckCommand::BeatJump(-16.0));
    rig.run(0.01);
    let d = rig.snap().decks[0];
    assert_eq!((d.loop_range, d.position), (Some((3 * sr, 5 * sr)), 3 * sr));
}

#[test]
fn sync_while_paused_matches_tempo_but_keeps_the_cue_position() {
    let mut rig = Rig::new(grid(120.0, 0.0), grid(124.0, 0.1));
    rig.seek(DeckId::A, 10.3);
    rig.send(DeckId::A, DeckCommand::Play);
    rig.seek(DeckId::B, 20.0);
    rig.run(0.2);
    rig.send(DeckId::B, DeckCommand::Sync);
    rig.run(0.2);
    let b = rig.snap().decks[1];
    assert_eq!(
        b.position,
        20 * u64::from(SR),
        "paused deck stays on its cue"
    );
    assert!((f64::from(TempoRange::Ten.rate(b.tempo)) * 124.0 - 120.0).abs() < 0.01);
}

/// Phase error of B against A in beats, right after a start.
fn start_error(rig: &mut Rig) -> f64 {
    wrap(rig.beat(DeckId::B) - rig.beat(DeckId::A))
}

#[test]
fn a_deck_synced_while_paused_starts_on_the_phase() {
    let mut rig = Rig::new(grid(120.0, 0.0), grid(124.0, 0.1));
    rig.seek(DeckId::A, 10.3);
    rig.send(DeckId::A, DeckCommand::Play);
    rig.seek(DeckId::B, 20.0);
    rig.run(0.2);
    rig.send(DeckId::B, DeckCommand::Sync);
    // A plays on for a while: the phase has to be taken at Play, not at Sync.
    rig.run(1.37);
    rig.send(DeckId::B, DeckCommand::Play);
    rig.run(0.05);
    let error = start_error(&mut rig);
    assert!(error.abs() < 0.015, "started {error} beats off");
    let b = rig.snap().decks[1].position as f64 / f64::from(SR);
    assert!((b - 20.0).abs() < 0.3, "moved more than half a beat: {b} s");
    // One-shot: the next start (after a pause) is not moved again.
    rig.send(DeckId::B, DeckCommand::Pause);
    rig.run(0.1);
    rig.seek(DeckId::B, 30.0);
    rig.run(0.01);
    rig.send(DeckId::B, DeckCommand::Play);
    rig.run(0.01);
    let b = rig.snap().decks[1].position;
    assert!(
        (30 * u64::from(SR)..30 * u64::from(SR) + 2 * BLOCK as u64).contains(&b),
        "a later start moved the deck: {b}"
    );
}

#[test]
fn a_locked_deck_starts_on_the_phase_instead_of_drifting_into_it() {
    let mut rig = Rig::new(grid(126.0, 0.05), grid(121.0, 0.21));
    rig.seek(DeckId::A, 7.0);
    rig.send(DeckId::A, DeckCommand::Play);
    rig.seek(DeckId::B, 12.0);
    rig.send(DeckId::B, DeckCommand::SyncLock(true));
    rig.run(0.83);
    rig.send(DeckId::B, DeckCommand::Play);
    rig.run(0.05);
    let error = start_error(&mut rig);
    assert!(error.abs() < 0.015, "started {error} beats off");
}

#[test]
fn previewing_the_cue_keeps_a_paused_sync_for_play() {
    let mut rig = Rig::new(grid(120.0, 0.0), grid(124.0, 0.1));
    rig.send(DeckId::A, DeckCommand::Play);
    rig.seek(DeckId::B, 20.0);
    rig.run(0.2);
    rig.send(DeckId::B, DeckCommand::Sync);
    rig.run(0.31);
    // The first press sets the cue point there, the second previews it.
    rig.send(DeckId::B, DeckCommand::CuePress);
    rig.send(DeckId::B, DeckCommand::CueRelease);
    rig.send(DeckId::B, DeckCommand::CuePress);
    rig.run(0.3);
    assert!(rig.snap().decks[1].playing, "not previewing");
    rig.send(DeckId::B, DeckCommand::CueRelease);
    rig.run(0.43);
    assert!(!rig.snap().decks[1].playing);
    rig.send(DeckId::B, DeckCommand::Play);
    rig.run(0.05);
    let error = start_error(&mut rig);
    assert!(
        error.abs() < 0.015,
        "started {error} beats off after a preview"
    );
}

#[test]
fn a_paused_sync_does_not_move_the_deck_when_the_other_deck_stands_still() {
    let mut rig = Rig::new(grid(120.0, 0.0), grid(124.0, 0.1));
    rig.seek(DeckId::A, 10.3);
    rig.seek(DeckId::B, 20.0);
    rig.run(0.01);
    rig.send(DeckId::B, DeckCommand::Sync);
    rig.send(DeckId::B, DeckCommand::Play);
    rig.run(0.01);
    let b = rig.snap().decks[1].position;
    assert!(
        (20 * u64::from(SR)..20 * u64::from(SR) + 2 * BLOCK as u64).contains(&b),
        "moved to {b}"
    );
}

fn at(rig: &mut Rig, deck: DeckId) -> u64 {
    rig.snap().decks[deck.index()].position
}

/// B synced while paused on its cue at `cue` s, A playing from 10.3 s.
fn paused_sync_rig(cue: f64) -> Rig {
    let mut rig = Rig::new(grid(120.0, 0.0), grid(124.0, 0.1));
    rig.seek(DeckId::A, 10.3);
    rig.send(DeckId::A, DeckCommand::Play);
    rig.seek(DeckId::B, cue);
    rig.run(0.2);
    rig.send(DeckId::B, DeckCommand::Sync);
    rig.run(0.41);
    rig
}

#[test]
fn loading_a_new_track_drops_a_pending_sync() {
    let mut rig = paused_sync_rig(20.0);
    let g = grid(124.0, 0.1);
    let load = TrackLoad {
        grid: Some(g),
        ..TrackLoad::new(DeckId::B, beat_track(3, g, 120.0))
    };
    assert!(rig.handle.load_track(load).is_ok());
    rig.run(0.01);
    rig.seek(DeckId::B, 30.0);
    rig.run(0.01);
    rig.send(DeckId::B, DeckCommand::Play);
    rig.run(0.01);
    let b = at(&mut rig, DeckId::B);
    let from = 30 * u64::from(SR);
    assert!((from..from + 2 * BLOCK as u64).contains(&b), "moved to {b}");
}

#[test]
fn play_while_holding_the_cue_catches_the_phase_once() {
    let mut rig = paused_sync_rig(20.0);
    let cue = 20 * u64::from(SR);
    rig.send(DeckId::B, DeckCommand::CuePress);
    rig.send(DeckId::B, DeckCommand::CueRelease);
    rig.send(DeckId::B, DeckCommand::CuePress);
    rig.run(0.01);
    // The preview plays the cue point itself, not a place near it.
    let b = at(&mut rig, DeckId::B);
    assert!(
        (cue..cue + 2 * BLOCK as u64).contains(&b),
        "preview from {b}"
    );
    rig.run(0.2);
    rig.send(DeckId::B, DeckCommand::Play);
    rig.send(DeckId::B, DeckCommand::CueRelease);
    rig.run(0.05);
    let error = start_error(&mut rig);
    assert!(error.abs() < 0.015, "joined {error} beats off");
    // Used up: a later start stays where it is.
    rig.send(DeckId::B, DeckCommand::Pause);
    rig.run(0.1);
    rig.seek(DeckId::B, 30.0);
    rig.run(0.01);
    rig.send(DeckId::B, DeckCommand::Play);
    rig.run(0.01);
    let b = at(&mut rig, DeckId::B);
    let from = 30 * u64::from(SR);
    assert!((from..from + 2 * BLOCK as u64).contains(&b), "moved to {b}");
}

#[test]
fn a_locked_preview_plays_the_cue_point_itself() {
    let mut rig = Rig::new(grid(120.0, 0.0), grid(124.0, 0.1));
    rig.send(DeckId::A, DeckCommand::Play);
    rig.seek(DeckId::B, 20.0);
    rig.send(DeckId::B, DeckCommand::SyncLock(true));
    rig.run(0.37);
    rig.send(DeckId::B, DeckCommand::CuePress);
    rig.send(DeckId::B, DeckCommand::CueRelease);
    rig.send(DeckId::B, DeckCommand::CuePress);
    rig.run(0.01);
    let b = at(&mut rig, DeckId::B);
    let cue = 20 * u64::from(SR);
    assert!(
        (cue..cue + 2 * BLOCK as u64).contains(&b),
        "preview from {b}"
    );
}

#[test]
fn a_start_never_leaves_a_running_loop() {
    let mut rig = paused_sync_rig(20.0);
    rig.send(DeckId::B, DeckCommand::BeatLoop(0.0625));
    rig.run(0.01);
    let (start, end) = rig.snap().decks[1].loop_range.unwrap();
    rig.send(DeckId::B, DeckCommand::Play);
    rig.run(0.003);
    let s = rig.snap().decks[1];
    assert_eq!(s.loop_range, Some((start, end)), "the start left the loop");
    assert!(
        (start..end).contains(&s.position),
        "{} outside the loop",
        s.position
    );
}

#[test]
fn a_deck_on_the_first_frame_still_starts_on_the_phase() {
    for a_at in [10.3, 10.45, 10.62, 10.81] {
        let mut rig = Rig::new(grid(120.0, 0.0), grid(124.0, 0.0));
        rig.seek(DeckId::A, a_at);
        rig.send(DeckId::A, DeckCommand::Play);
        rig.seek(DeckId::B, 0.0);
        rig.run(0.1);
        rig.send(DeckId::B, DeckCommand::Sync);
        rig.send(DeckId::B, DeckCommand::Play);
        rig.run(0.05);
        let error = start_error(&mut rig);
        assert!(
            error.abs() < 0.015,
            "A at {a_at}: started {error} beats off"
        );
    }
}

#[test]
fn the_leading_deck_is_not_moved_when_it_starts() {
    let mut rig = Rig::new(grid(120.0, 0.0), grid(124.0, 0.1));
    rig.seek(DeckId::B, 20.0);
    rig.send(DeckId::B, DeckCommand::Play);
    rig.seek(DeckId::A, 10.3);
    rig.send(DeckId::A, DeckCommand::SyncLock(true));
    rig.run(0.05);
    // B locks later and so follows A, even while A stands still.
    rig.send(DeckId::B, DeckCommand::SyncLock(true));
    rig.run(0.33);
    rig.send(DeckId::A, DeckCommand::Play);
    rig.run(0.01);
    let a = at(&mut rig, DeckId::A);
    let from = (10.3 * f64::from(SR)) as u64;
    // A plays at B's tempo (×1.033) after locking: allow three blocks.
    assert!(
        (from..from + 3 * BLOCK as u64).contains(&a),
        "leader moved to {a}"
    );
}

#[test]
fn unloading_a_deck_releases_its_sync_lock() {
    let mut rig = Rig::new(grid(120.0, 0.0), grid(124.0, 0.0));
    rig.send(DeckId::B, DeckCommand::SyncLock(true));
    rig.run(0.01);
    assert!(rig.snap().decks[1].sync_lock);
    rig.send(DeckId::B, DeckCommand::Unload);
    rig.run(0.01);
    assert!(!rig.snap().decks[1].sync_lock);
}

#[test]
fn loops_resize_from_their_start_and_beat_jump_moves_them() {
    let g = grid(120.0, 0.0);
    let mut rig = Rig::new(g, g);
    rig.seek(DeckId::A, 30.0);
    rig.send(DeckId::A, DeckCommand::BeatLoop(8.0));
    rig.send(DeckId::A, DeckCommand::BeatLoop(2.0));
    rig.run(0.01);
    let sr = u64::from(SR);
    assert_eq!(rig.snap().decks[0].loop_range, Some((30 * sr, 31 * sr)));
    rig.send(DeckId::A, DeckCommand::BeatJump(4.0));
    rig.run(0.01);
    let d = rig.snap().decks[0];
    assert_eq!(d.loop_range, Some((32 * sr, 33 * sr)));
    assert_eq!(d.position, 32 * sr);
    rig.send(DeckId::A, DeckCommand::LoopExit);
    rig.send(DeckId::A, DeckCommand::BeatJump(-0.5));
    rig.run(0.01);
    let d = rig.snap().decks[0];
    assert_eq!((d.loop_range, d.position), (None, 32 * sr - sr / 4));
}

#[test]
fn hot_cues_set_on_the_beat_jump_and_clear() {
    let g = grid(120.0, 0.0);
    let mut rig = Rig::new(g, g);
    let sr = u64::from(SR);
    rig.send(
        DeckId::A,
        DeckCommand::Seek {
            frame: 8 * sr + 1_000,
        },
    );
    rig.send(DeckId::A, DeckCommand::HotCue(3));
    rig.run(0.01);
    assert_eq!(
        rig.snap().decks[0].hot_cues[3],
        Some(8 * sr),
        "snapped to the beat"
    );
    rig.seek(DeckId::A, 40.0);
    rig.send(DeckId::A, DeckCommand::HotCue(3));
    rig.run(0.01);
    assert_eq!(rig.snap().decks[0].position, 8 * sr);
    rig.send(DeckId::A, DeckCommand::HotCueClear(3));
    rig.send(DeckId::A, DeckCommand::HotCue(200)); // out of range: ignored
    rig.run(0.01);
    assert_eq!(rig.snap().decks[0].hot_cues, [None; HOT_CUES]);
}

#[test]
fn without_a_grid_there_are_no_beat_loops_or_jumps_but_cues_work() {
    let (mut engine, mut handle) = engine_pair(SR, BLOCK);
    let g = grid(120.0, 0.0);
    assert!(handle.load(DeckId::A, beat_track(1, g, 20.0)).is_ok());
    let mut out = vec![0.0f32; BLOCK * 2];
    handle.send(Command::Deck(
        DeckId::A,
        DeckCommand::Seek { frame: 100_000 },
    ));
    for cmd in [
        DeckCommand::BeatLoop(4.0),
        DeckCommand::BeatJump(4.0),
        DeckCommand::Sync,
        DeckCommand::HotCue(0),
    ] {
        handle.send(Command::Deck(DeckId::A, cmd));
    }
    engine.process(&mut out, 2);
    let d = handle.snapshot().decks[0];
    assert_eq!((d.loop_range, d.position), (None, 100_000));
    assert_eq!(d.hot_cues[0], Some(100_000), "unquantised without a grid");
}

#[test]
fn at_a_crooked_tempo_the_loop_keeps_its_exact_length_on_average() {
    // At +3.7 % a one-beat loop lasts 24000 / 1.037 = 23143.7 output frames: some passes are
    // one frame longer than others, but the overshoot past the loop end must be carried into
    // the next pass, or the loop would run short by up to a frame every time.
    let g = grid(120.0, 0.0);
    let (mut engine, mut handle) = engine_pair(SR, BLOCK);
    let load = TrackLoad {
        grid: Some(g),
        ..TrackLoad::new(DeckId::A, beat_track(1, g, 30.0))
    };
    assert!(handle.load_track(load).is_ok());
    handle.send(Command::Deck(
        DeckId::A,
        DeckCommand::TempoRange(TempoRange::Ten),
    ));
    handle.send(Command::Deck(DeckId::A, DeckCommand::Tempo(0.37)));
    handle.send(Command::Deck(
        DeckId::A,
        DeckCommand::Seek {
            frame: 5 * 48_000 + 100,
        },
    ));
    handle.send(Command::Deck(DeckId::A, DeckCommand::Play));
    handle.send(Command::Deck(DeckId::A, DeckCommand::BeatLoop(1.0)));
    let mut audio = Vec::new();
    let mut out = vec![0.0f32; BLOCK * 2];
    for _ in 0..(13 * SR as usize / BLOCK) {
        engine.process(&mut out, 2);
        audio.extend(out.as_chunks::<2>().0.iter().map(|f| f[0]));
    }
    // The click at the loop start marks every pass.
    let mut onsets = Vec::new();
    let mut k = 1_000;
    while k < audio.len() {
        if audio[k] > 0.5 && audio[k - 1] <= 0.5 {
            onsets.push(k);
            k += 10_000;
        } else {
            k += 1;
        }
    }
    assert!(onsets.len() >= 20, "{} passes", onsets.len());
    let (first, last) = (onsets[1], onsets[onsets.len() - 1]);
    let mean = (last - first) as f64 / (onsets.len() - 2) as f64;
    let expected = 24_000.0 / f64::from(TempoRange::Ten.rate(0.37));
    assert!(
        (mean - expected).abs() < 0.05,
        "mean pass {mean}, expected {expected}"
    );
}

/// Frequency from interpolated rising zero crossings (first to last crossing in `v`).
fn precise_frequency(v: &[f32]) -> f64 {
    let crossings: Vec<f64> = v
        .windows(2)
        .enumerate()
        .filter(|(_, w)| w[0] < 0.0 && w[1] >= 0.0)
        .map(|(i, w)| i as f64 + f64::from(w[0] / (w[0] - w[1])))
        .collect();
    let (first, last) = (crossings[0], crossings[crossings.len() - 1]);
    (crossings.len() - 1) as f64 * f64::from(SR) / (last - first)
}

/// A 440 Hz tone with a 120 BPM grid, keylock on at +8 %, playing a one-beat loop at 5 s.
fn keylock_loop_rig() -> (Engine, EngineHandle) {
    let g = grid(120.0, 0.0);
    let (mut engine, mut handle) = engine_pair(SR, BLOCK);
    let frames = 30 * SR as usize;
    let samples = (0..frames)
        .flat_map(|i| {
            let v = 0.5 * (i as f32 * 440.0 * std::f32::consts::TAU / SR as f32).sin();
            [v, v]
        })
        .collect();
    let load = TrackLoad {
        grid: Some(g),
        ..TrackLoad::new(DeckId::A, Arc::new(TrackAudio::new(1, SR, samples)))
    };
    assert!(handle.load_track(load).is_ok());
    let a = |c| Command::Deck(DeckId::A, c);
    handle.send(a(DeckCommand::TempoRange(TempoRange::Ten)));
    handle.send(a(DeckCommand::Tempo(0.8)));
    handle.send(a(DeckCommand::Keylock(true)));
    handle.send(a(DeckCommand::Seek { frame: 5 * 48_000 }));
    handle.send(a(DeckCommand::Play));
    let mut out = vec![0.0f32; BLOCK * 2];
    for _ in 0..(SR as usize / BLOCK) {
        engine.process(&mut out, 2);
    }
    handle.send(a(DeckCommand::BeatLoop(1.0)));
    (engine, handle)
}

#[test]
fn a_keylock_loop_keeps_the_pitch_right_after_the_seam() {
    // With keylock every pass must sound at the track's pitch, from its first frame. Before,
    // each pass re-engaged the stretcher and opened with a varispeed bridge, i.e. briefly at
    // the tempo's pitch (+8 % here).
    let (mut engine, mut handle) = keylock_loop_rig();
    let mut out = vec![0.0f32; BLOCK * 2];
    let mut audio = Vec::new();
    let mut seams = Vec::new();
    let mut last = handle.snapshot().decks[0].position;
    for _ in 0..(4 * SR as usize / BLOCK) {
        engine.process(&mut out, 2);
        audio.extend(out.as_chunks::<2>().0.iter().map(|f| f[0]));
        let pos = handle.snapshot().decks[0].position;
        if pos < last {
            seams.push(audio.len() - BLOCK);
        }
        last = pos;
    }
    assert!(seams.len() >= 4, "the loop ran: {seams:?}");
    for &s in &seams[1..seams.len() - 1] {
        let f = precise_frequency(&audio[s + BLOCK + 300..s + BLOCK + 1500]);
        assert!((f - 440.0).abs() < 4.0, "seam at {s}: {f} Hz");
    }
}

#[test]
fn leaving_a_keylock_loop_early_in_a_pass_keeps_the_pitch() {
    // Early in a pass the stretcher has not been fed the next one yet: it plays on into the
    // track instead of restarting (which would open with the tempo's pitch again).
    let (mut engine, mut handle) = keylock_loop_rig();
    let mut out = vec![0.0f32; BLOCK * 2];
    let mut last = handle.snapshot().decks[0].position;
    let mut seams = 0;
    while seams < 2 {
        engine.process(&mut out, 2);
        let pos = handle.snapshot().decks[0].position;
        seams += usize::from(pos < last);
        last = pos;
    }
    handle.send(Command::Deck(DeckId::A, DeckCommand::LoopExit));
    let mut audio = Vec::new();
    for _ in 0..(SR as usize / BLOCK) {
        engine.process(&mut out, 2);
        audio.extend(out.as_chunks::<2>().0.iter().map(|f| f[0]));
    }
    let d = handle.snapshot().decks[0];
    assert_eq!(d.loop_range, None);
    for w in [0, 1200, 2400, 3600] {
        let f = precise_frequency(&audio[w + 300..w + 1500]);
        assert!((f - 440.0).abs() < 4.0, "{w} frames after leaving: {f} Hz");
    }
}
