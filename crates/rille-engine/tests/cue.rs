#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code

//! The headphone (cue) bus: pre-fader taps, master cue, blend and level, and its two routes
//! (channels 3/4 of the main device, or the ring for a second device).

use std::sync::Arc;

use rille_core::{Command, CueRoute, DeckCommand, DeckId, MixerCommand, TrackAudio};
use rille_engine::cue::{DriftReader, FrameSource};
use rille_engine::{Engine, EngineHandle, engine_pair};

const SR: u32 = 48_000;
const BLOCK: usize = 256;

/// Deck A plays a constant 0.5, deck B a constant 0.25 (easy to tell apart in the sums).
fn rig() -> (Engine, EngineHandle) {
    let (engine, mut h) = engine_pair(SR, BLOCK);
    let dc = |id, v| Arc::new(TrackAudio::new(id, SR, vec![v; SR as usize * 2 * 20]));
    assert!(h.load(DeckId::A, dc(1, 0.5)).is_ok());
    assert!(h.load(DeckId::B, dc(2, 0.25)).is_ok());
    h.send(Command::Deck(DeckId::A, DeckCommand::Play));
    h.send(Command::Deck(DeckId::B, DeckCommand::Play));
    (engine, h)
}

fn mix(h: &mut EngineHandle, c: MixerCommand) {
    h.send(Command::Mixer(c));
}

/// Render half a second into a 4-channel buffer and return the last frame.
fn last_frame(engine: &mut Engine) -> [f32; 4] {
    let mut out = vec![0.0f32; BLOCK * 4];
    for _ in 0..(SR as usize / 2 / BLOCK) {
        engine.process(&mut out, 4);
    }
    let n = out.len();
    [out[n - 4], out[n - 3], out[n - 2], out[n - 1]]
}

#[test]
fn the_cue_bus_is_taken_before_the_channel_fader() {
    let (mut engine, mut h) = rig();
    mix(&mut h, MixerCommand::CueRoute(CueRoute::Channels34));
    mix(&mut h, MixerCommand::CueMix(0.0)); // cue only
    mix(&mut h, MixerCommand::CueLevel(1.0));
    mix(&mut h, MixerCommand::ChannelFader(DeckId::A, 0.0));
    mix(&mut h, MixerCommand::ChannelFader(DeckId::B, 0.0));
    mix(&mut h, MixerCommand::HeadphoneCue(DeckId::A, true));
    let f = last_frame(&mut engine);
    assert!(
        f[0].abs() < 1e-4,
        "master silent with both faders down: {f:?}"
    );
    assert!(
        (f[2] - 0.5).abs() < 1e-3,
        "deck A in the headphones at unity: {f:?}"
    );
    mix(&mut h, MixerCommand::HeadphoneCue(DeckId::B, true));
    let f = last_frame(&mut engine);
    assert!((f[2] - 0.75).abs() < 1e-3, "both cued: {f:?}");
    assert!((f[3] - f[2]).abs() < 1e-6);
    let s = h.snapshot();
    assert_eq!((s.cue, s.cue_route), ([true, true], CueRoute::Channels34));
    assert!(s.cue_peak[0] > 0.7);
}

#[test]
fn mix_and_master_cue_blend_the_master_in() {
    let (mut engine, mut h) = rig();
    mix(&mut h, MixerCommand::CueRoute(CueRoute::Channels34));
    mix(&mut h, MixerCommand::CueLevel(1.0));
    let master = last_frame(&mut engine)[0];
    assert!(master > 0.1);
    // Mix fully to master: the headphones hear the master, cue or not.
    mix(&mut h, MixerCommand::CueMix(1.0));
    let f = last_frame(&mut engine);
    assert!((f[2] - master).abs() < 1e-3, "{f:?} vs master {master}");
    // Cue only, nothing cued: silence; with master cue on, the master again.
    mix(&mut h, MixerCommand::CueMix(0.0));
    assert!(last_frame(&mut engine)[2].abs() < 1e-4);
    mix(&mut h, MixerCommand::MasterCue(true));
    let f = last_frame(&mut engine);
    assert!((f[2] - master).abs() < 1e-3, "{f:?}");
    // Level down: silence in the headphones only.
    mix(&mut h, MixerCommand::CueLevel(0.0));
    let f = last_frame(&mut engine);
    assert!(f[2].abs() < 1e-4 && (f[0] - master).abs() < 1e-3);
}

#[test]
fn switching_cue_on_and_off_does_not_click() {
    let (mut engine, mut h) = rig();
    mix(&mut h, MixerCommand::CueRoute(CueRoute::Channels34));
    mix(&mut h, MixerCommand::CueMix(0.0));
    mix(&mut h, MixerCommand::CueLevel(1.0));
    let mut out = vec![0.0f32; BLOCK * 4];
    for _ in 0..40 {
        engine.process(&mut out, 4); // settle the start ramps first
    }
    mix(&mut h, MixerCommand::HeadphoneCue(DeckId::A, true));
    let mut prev = out[out.len() - 2];
    let mut worst = 0.0f32;
    for _ in 0..20 {
        engine.process(&mut out, 4);
        for f in out.as_chunks::<4>().0 {
            worst = worst.max((f[2] - prev).abs());
            prev = f[2];
        }
    }
    assert!(worst < 0.01, "step of {worst} when switching cue on");
}

#[test]
fn channels_3_and_4_stay_silent_unless_routed_there() {
    let (mut engine, mut h) = rig();
    mix(&mut h, MixerCommand::HeadphoneCue(DeckId::A, true));
    mix(&mut h, MixerCommand::CueMix(0.0));
    for route in [CueRoute::Off, CueRoute::Ring] {
        mix(&mut h, MixerCommand::CueRoute(route));
        let f = last_frame(&mut engine);
        assert_eq!((f[2], f[3]), (0.0, 0.0), "{route:?}");
    }
}

#[test]
fn the_ring_is_filled_only_for_a_second_device_and_never_blocks() {
    let (mut engine, mut h) = rig();
    let reader = h.take_cue_reader().unwrap();
    assert!(h.take_cue_reader().is_none(), "one reader only");
    mix(&mut h, MixerCommand::HeadphoneCue(DeckId::A, true));
    mix(&mut h, MixerCommand::CueMix(0.0));
    mix(&mut h, MixerCommand::CueLevel(1.0));
    let mut out = vec![0.0f32; BLOCK * 2];
    engine.process(&mut out, 2);
    assert_eq!(reader.frames_available(), 0, "route off: nothing written");
    mix(&mut h, MixerCommand::CueRoute(CueRoute::Ring));
    // Nobody reads: the ring fills up, the engine keeps rendering the master.
    for _ in 0..200 {
        engine.process(&mut out, 2);
    }
    assert!(reader.frames_available() > 10_000);
    assert!(out[0] > 0.1, "master still playing");
    // A second device reading through the drift reader hears deck A. Opening the device
    // drops the backlog (`restart`), as the app does.
    let mut dr = DriftReader::new(reader, 512);
    dr.restart(512);
    for _ in 0..4 {
        engine.process(&mut out, 2);
    }
    let mut hp = vec![0.0f32; 256 * 2];
    dr.render(&mut hp, 2);
    dr.render(&mut hp, 2);
    assert!((hp[200] - 0.5).abs() < 1e-3, "{}", hp[200]);
}

#[test]
fn controller_buttons_toggle_the_cue_state() {
    let (mut engine, mut h) = rig();
    mix(&mut h, MixerCommand::ToggleHeadphoneCue(DeckId::B));
    mix(&mut h, MixerCommand::ToggleMasterCue);
    last_frame(&mut engine);
    let s = h.snapshot();
    assert_eq!((s.cue, s.master_cue), ([false, true], true));
    mix(&mut h, MixerCommand::ToggleHeadphoneCue(DeckId::B));
    last_frame(&mut engine);
    assert_eq!(h.snapshot().cue, [false, false]);
}
