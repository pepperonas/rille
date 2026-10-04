#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code

//! The audio callback must never allocate. With the `warn_debug` feature, `assert_no_alloc`
//! counts violations instead of aborting, so this test can also prove that it would notice.

use std::sync::Arc;

use assert_no_alloc::{AllocDisabler, assert_no_alloc, reset_violation_count, violation_count};
use rille_core::{Command, DeckCommand, DeckId, EqBand, MixerCommand, TrackAudio};
use rille_engine::engine_pair;

#[global_allocator]
static ALLOC: AllocDisabler = AllocDisabler;

const SR: u32 = 48_000;

#[test]
fn process_never_allocates() {
    // Single test function: violation_count is global, parallel tests would mix counts.
    reset_violation_count();

    // Counter-check first: an allocation inside the guard is detected.
    assert_no_alloc(|| {
        let v: Vec<u8> = Vec::with_capacity(16);
        std::hint::black_box(v);
    });
    assert!(
        violation_count() > 0,
        "assert_no_alloc does not detect allocations"
    );
    reset_violation_count();

    let (mut engine, mut handle) = engine_pair(SR, 256);
    let track = |id| Arc::new(TrackAudio::new(id, SR, vec![0.25; SR as usize * 2 * 5]));
    handle.load(DeckId::A, track(1)).map_err(|_| ()).unwrap();
    handle.load(DeckId::B, track(2)).map_err(|_| ()).unwrap();
    let mut out = vec![0.0f32; 512 * 2];

    for block in 0..2000u32 {
        match block % 50 {
            0 => {
                handle.send(Command::Deck(DeckId::A, DeckCommand::PlayPause));
                handle.send(Command::Deck(DeckId::B, DeckCommand::Play));
            }
            10 => {
                handle.send(Command::Mixer(MixerCommand::Crossfader(
                    (block % 7) as f32 / 6.0,
                )));
                handle.send(Command::Mixer(MixerCommand::ChannelFader(DeckId::A, 0.3)));
            }
            20 => {
                handle.send(Command::Deck(DeckId::A, DeckCommand::CuePress));
                handle.send(Command::Deck(DeckId::A, DeckCommand::CueRelease));
            }
            30 => {
                // Swapping a track hands the old one to the garbage queue (no free in process).
                let _ = handle.load(DeckId::B, track(u64::from(block)));
            }
            40 => {
                handle.send(Command::Deck(DeckId::B, DeckCommand::Unload));
            }
            5 => {
                let v = (block % 11) as f32 / 10.0;
                handle.send(Command::Mixer(MixerCommand::Eq(DeckId::A, EqBand::Mid, v)));
                handle.send(Command::Mixer(MixerCommand::EqKill(
                    DeckId::A,
                    EqBand::Low,
                    block % 100 == 5,
                )));
                handle.send(Command::Mixer(MixerCommand::Filter(DeckId::A, 1.0 - v)));
            }
            15 => {
                // Start or cancel a transition effect; cycle the kind now and then.
                handle.send(Command::Mixer(MixerCommand::TransitionFx));
                if block % 200 == 15 {
                    handle.send(Command::Mixer(MixerCommand::CycleTransitionFx));
                }
            }
            _ => {}
        }
        // Device buffer of 512 frames with max block 256 exercises the splitting path.
        assert_no_alloc(|| engine.process(&mut out, 2));
        handle.collect_garbage();
        let _ = handle.snapshot();
        while handle.next_event().is_some() {}
    }

    assert_eq!(violation_count(), 0, "engine.process allocated");
}
