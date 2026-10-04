//! The realtime engine: decks + mixer, rendered block by block.
//!
//! [`Engine::process`] is the only entry point from the audio thread. It must stay free of
//! allocation, locks, logging and I/O. Everything it needs is allocated in [`engine_pair`].

use std::sync::Arc;

use rille_core::{
    Command, CrossfaderCurve, DeckCommand, DeckId, DeckSnapshot, MixerCommand, Snapshot, TrackAudio,
};
use rtrb::{Consumer, Producer, RingBuffer};
use triple_buffer::{Input, Output};

use crate::curves::{channel_gain, crossfader_gains, trim_gain};
use crate::meter::PeakMeter;
use crate::smooth::Smoother;
use crate::transport::Transport;

/// Capacity of each command queue. A DDJ-200 jog sends ~1 kHz at most; 1024 covers several
/// blocks of backlog.
pub const COMMAND_QUEUE: usize = 1024;
/// Commands drained per block, so a flood can never stall the callback.
pub const COMMANDS_PER_BLOCK: usize = 256;
const TRACK_QUEUE: usize = 8;
const GARBAGE_QUEUE: usize = 16;
const EVENT_QUEUE: usize = 64;
const PENDING_DROPS: usize = 8;
/// Smoothing time constant for levels.
pub const LEVEL_SMOOTHING_MS: f32 = 8.0;

/// A decoded track on its way to a deck. Replacing a track hands the old one back through the
/// garbage queue, so the audio thread never frees memory.
pub struct TrackLoad {
    pub deck: DeckId,
    pub track: Arc<TrackAudio>,
}

/// Things the engine reports that must not get lost (unlike the snapshot, which is overwritten).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineEvent {
    TrackLoaded { deck: DeckId, track_id: u64 },
    TrackEnded { deck: DeckId },
}

/// Engine-side ends of all queues.
pub struct EngineIo {
    ui_rx: Consumer<Command>,
    midi_rx: Consumer<Command>,
    track_rx: Consumer<TrackLoad>,
    garbage_tx: Producer<Arc<TrackAudio>>,
    events_tx: Producer<EngineEvent>,
    snapshot_tx: Input<Snapshot>,
}

/// Control-side ends of all queues. Lives outside the audio thread.
pub struct EngineHandle {
    ui_tx: Producer<Command>,
    /// Taken by the MIDI layer, which then is the only producer on this queue.
    pub midi_tx: Option<Producer<Command>>,
    track_tx: Producer<TrackLoad>,
    garbage_rx: Consumer<Arc<TrackAudio>>,
    events_rx: Consumer<EngineEvent>,
    snapshot_rx: Output<Snapshot>,
    dropped_commands: u32,
}

impl EngineHandle {
    /// Queue a command from the UI. Returns `false` (and counts it) when the queue is full.
    pub fn send(&mut self, command: Command) -> bool {
        if self.ui_tx.push(command).is_ok() {
            true
        } else {
            self.dropped_commands = self.dropped_commands.saturating_add(1);
            false
        }
    }

    pub fn dropped_commands(&self) -> u32 {
        self.dropped_commands
    }

    /// Hand a decoded track to a deck. Gives the track back if the queue is full.
    pub fn load(&mut self, deck: DeckId, track: Arc<TrackAudio>) -> Result<(), Arc<TrackAudio>> {
        self.track_tx
            .push(TrackLoad { deck, track })
            .map_err(|e| match e {
                rtrb::PushError::Full(load) => load.track,
            })
    }

    /// Latest engine state. Cheap; call at UI rate.
    pub fn snapshot(&mut self) -> Snapshot {
        *self.snapshot_rx.read()
    }

    /// Free tracks the engine no longer uses. Call regularly from a non-realtime thread.
    pub fn collect_garbage(&mut self) -> usize {
        let mut n = 0;
        while let Ok(track) = self.garbage_rx.pop() {
            drop(track);
            n += 1;
        }
        n
    }

    pub fn next_event(&mut self) -> Option<EngineEvent> {
        self.events_rx.pop().ok()
    }
}

/// Create an engine and its control handle. All memory the engine will ever use is allocated
/// here.
pub fn engine_pair(sample_rate: u32, max_block: usize) -> (Engine, EngineHandle) {
    let (ui_tx, ui_rx) = RingBuffer::new(COMMAND_QUEUE);
    let (midi_tx, midi_rx) = RingBuffer::new(COMMAND_QUEUE);
    let (track_tx, track_rx) = RingBuffer::new(TRACK_QUEUE);
    let (garbage_tx, garbage_rx) = RingBuffer::new(GARBAGE_QUEUE);
    let (events_tx, events_rx) = RingBuffer::new(EVENT_QUEUE);
    let initial = Snapshot {
        sample_rate,
        ..Engine::initial_snapshot()
    };
    let (snapshot_tx, snapshot_rx) = triple_buffer::triple_buffer(&initial);
    let io = EngineIo {
        ui_rx,
        midi_rx,
        track_rx,
        garbage_tx,
        events_tx,
        snapshot_tx,
    };
    let handle = EngineHandle {
        ui_tx,
        midi_tx: Some(midi_tx),
        track_tx,
        garbage_rx,
        events_rx,
        snapshot_rx,
        dropped_commands: 0,
    };
    (Engine::new(sample_rate, max_block, io), handle)
}

struct Deck {
    track: Option<Arc<TrackAudio>>,
    transport: Transport,
    /// Rendered stereo frames for the current block (interleaved), sized for `max_block`.
    buffer: Vec<f32>,
    gain: Smoother,
    meter: PeakMeter,
}

impl Deck {
    fn new(sample_rate: u32, max_block: usize) -> Deck {
        Deck {
            track: None,
            transport: Transport::new(0),
            buffer: vec![0.0; max_block * 2],
            gain: Smoother::new(sample_rate, LEVEL_SMOOTHING_MS, 0.0),
            meter: PeakMeter::new(sample_rate),
        }
    }

    /// Render `frames` frames into `self.buffer`. Returns `true` when the track ended.
    fn render(&mut self, frames: usize) -> bool {
        let out = &mut self.buffer[..frames * 2];
        match &self.track {
            Some(track) if self.transport.playing => {
                let start = self.transport.position as usize;
                for (i, frame) in out.as_chunks_mut::<2>().0.iter_mut().enumerate() {
                    let [l, r] = track.frame(start + i);
                    frame[0] = l;
                    frame[1] = r;
                }
                self.transport.advance(frames as u64)
            }
            _ => {
                out.fill(0.0);
                false
            }
        }
    }
}

pub struct Engine {
    sample_rate: u32,
    max_block: usize,
    io: EngineIo,
    decks: [Deck; 2],
    channel_fader: [f32; 2],
    trim: [f32; 2],
    crossfader: f32,
    curve: CrossfaderCurve,
    master_target: f32,
    master_gain: Smoother,
    master_meter: PeakMeter,
    pending_drops: [Option<Arc<TrackAudio>>; PENDING_DROPS],
    frame_clock: u64,
    xruns: u32,
}

impl Engine {
    fn initial_snapshot() -> Snapshot {
        Snapshot {
            channel_fader: [1.0; 2],
            trim: [0.5; 2],
            crossfader: 0.5,
            master_gain: 1.0,
            ..Snapshot::default()
        }
    }

    fn new(sample_rate: u32, max_block: usize, io: EngineIo) -> Engine {
        let defaults = Self::initial_snapshot();
        let max_block = max_block.max(1);
        let mut engine = Engine {
            sample_rate,
            max_block,
            io,
            decks: [
                Deck::new(sample_rate, max_block),
                Deck::new(sample_rate, max_block),
            ],
            channel_fader: defaults.channel_fader,
            trim: defaults.trim,
            crossfader: defaults.crossfader,
            curve: defaults.curve,
            master_target: defaults.master_gain,
            master_gain: Smoother::new(sample_rate, LEVEL_SMOOTHING_MS, defaults.master_gain),
            master_meter: PeakMeter::new(sample_rate),
            pending_drops: Default::default(),
            frame_clock: 0,
            xruns: 0,
        };
        engine.update_gain_targets();
        for deck in &mut engine.decks {
            deck.gain.snap();
        }
        engine
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn max_block(&self) -> usize {
        self.max_block
    }

    /// Called by the output layer when it detects a late callback.
    pub fn note_xrun(&mut self) {
        self.xruns = self.xruns.saturating_add(1);
    }

    /// Render into an interleaved device buffer with `channels` channels. Channels 0/1 get the
    /// master; any further channels are silenced (cue routing comes later).
    pub fn process(&mut self, out: &mut [f32], channels: usize) {
        if channels == 0 {
            return;
        }
        let total = out.len() / channels;
        let mut done = 0;
        while done < total {
            let frames = (total - done).min(self.max_block);
            let chunk = &mut out[done * channels..(done + frames) * channels];
            self.process_block(chunk, channels, frames);
            done += frames;
        }
        self.publish_snapshot();
    }

    fn process_block(&mut self, out: &mut [f32], channels: usize, frames: usize) {
        self.drain_queues();
        self.retry_pending_drops();

        for (index, deck) in self.decks.iter_mut().enumerate() {
            if deck.render(frames)
                && let Some(id) = DeckId::from_index(index)
            {
                let _ = self.io.events_tx.push(EngineEvent::TrackEnded { deck: id });
            }
        }

        let [a, b] = &mut self.decks;
        let mut deck_peak = [[0.0f32; 2]; 2];
        let mut master_peak = [0.0f32; 2];
        for i in 0..frames {
            let ga = a.gain.tick();
            let gb = b.gain.tick();
            let gm = self.master_gain.tick();
            let (al, ar) = (a.buffer[2 * i] * ga, a.buffer[2 * i + 1] * ga);
            let (bl, br) = (b.buffer[2 * i] * gb, b.buffer[2 * i + 1] * gb);
            let l = (al + bl) * gm;
            let r = (ar + br) * gm;
            deck_peak[0] = [deck_peak[0][0].max(al.abs()), deck_peak[0][1].max(ar.abs())];
            deck_peak[1] = [deck_peak[1][0].max(bl.abs()), deck_peak[1][1].max(br.abs())];
            master_peak = [master_peak[0].max(l.abs()), master_peak[1].max(r.abs())];
            let frame = &mut out[i * channels..(i + 1) * channels];
            frame[0] = l;
            if channels > 1 {
                frame[1] = r;
                frame[2..].fill(0.0);
            }
        }
        a.meter.update(deck_peak[0], frames);
        b.meter.update(deck_peak[1], frames);
        self.master_meter.update(master_peak, frames);
        self.frame_clock += frames as u64;
    }

    fn drain_queues(&mut self) {
        while let Ok(load) = self.io.track_rx.pop() {
            self.install_track(load);
        }
        for _ in 0..COMMANDS_PER_BLOCK {
            let Ok(command) = self.io.ui_rx.pop() else {
                break;
            };
            self.apply(command);
        }
        for _ in 0..COMMANDS_PER_BLOCK {
            let Ok(command) = self.io.midi_rx.pop() else {
                break;
            };
            self.apply(command);
        }
    }

    fn install_track(&mut self, load: TrackLoad) {
        let deck = &mut self.decks[load.deck.index()];
        let track_id = load.track.id;
        deck.transport = Transport::new(load.track.frames() as u64);
        let old = deck.track.replace(load.track);
        self.release(old);
        let _ = self.io.events_tx.push(EngineEvent::TrackLoaded {
            deck: load.deck,
            track_id,
        });
    }

    /// Hand a track back to the control side without dropping it here.
    fn release(&mut self, track: Option<Arc<TrackAudio>>) {
        let Some(track) = track else { return };
        if let Err(rtrb::PushError::Full(track)) = self.io.garbage_tx.push(track) {
            if let Some(slot) = self.pending_drops.iter_mut().find(|s| s.is_none()) {
                *slot = Some(track);
            } else {
                // Every slot is taken and the control side is not collecting. Leaking one track
                // is the lesser evil compared with freeing memory on the audio thread.
                std::mem::forget(track);
            }
        }
    }

    fn retry_pending_drops(&mut self) {
        for slot in &mut self.pending_drops {
            if let Some(track) = slot.take()
                && let Err(rtrb::PushError::Full(track)) = self.io.garbage_tx.push(track)
            {
                *slot = Some(track);
                return;
            }
        }
    }

    fn apply(&mut self, command: Command) {
        match command {
            Command::Deck(id, cmd) => self.apply_deck(id, cmd),
            Command::Mixer(cmd) => self.apply_mixer(cmd),
        }
    }

    fn apply_deck(&mut self, id: DeckId, cmd: DeckCommand) {
        if cmd == DeckCommand::Unload {
            let deck = &mut self.decks[id.index()];
            deck.transport = Transport::new(0);
            let old = deck.track.take();
            self.release(old);
            return;
        }
        let t = &mut self.decks[id.index()].transport;
        match cmd {
            DeckCommand::PlayPause => t.play_pause(),
            DeckCommand::Play => t.play(),
            DeckCommand::Pause => t.pause(),
            DeckCommand::CuePress => t.cue_press(),
            DeckCommand::CueRelease => t.cue_release(),
            DeckCommand::JumpToStart => t.jump_to_start(),
            DeckCommand::Seek { frame } => t.seek(frame),
            DeckCommand::Unload => {}
        }
    }

    fn apply_mixer(&mut self, cmd: MixerCommand) {
        let unit = |v: f32| {
            if v.is_finite() {
                v.clamp(0.0, 1.0)
            } else {
                0.0
            }
        };
        match cmd {
            MixerCommand::ChannelFader(id, v) => self.channel_fader[id.index()] = unit(v),
            MixerCommand::Trim(id, v) => self.trim[id.index()] = unit(v),
            MixerCommand::Crossfader(v) => self.crossfader = unit(v),
            MixerCommand::CrossfaderCurve(c) => self.curve = c,
            MixerCommand::MasterGain(v) => {
                self.master_target = unit(v);
                self.master_gain.set_target(self.master_target);
            }
        }
        self.update_gain_targets();
    }

    fn update_gain_targets(&mut self) {
        let (xa, xb) = crossfader_gains(self.crossfader, self.curve);
        let xf = [xa, xb];
        for (i, deck) in self.decks.iter_mut().enumerate() {
            deck.gain
                .set_target(trim_gain(self.trim[i]) * channel_gain(self.channel_fader[i]) * xf[i]);
        }
    }

    fn publish_snapshot(&mut self) {
        let deck = |d: &Deck| DeckSnapshot {
            track_id: d.track.as_ref().map(|t| t.id),
            position: d.transport.position,
            frames: d.transport.frames,
            playing: d.transport.playing,
            cue: d.transport.cue,
            previewing: d.transport.previewing,
            peak: d.meter.value(),
        };
        let snapshot = Snapshot {
            frame_clock: self.frame_clock,
            sample_rate: self.sample_rate,
            decks: [deck(&self.decks[0]), deck(&self.decks[1])],
            channel_fader: self.channel_fader,
            trim: self.trim,
            crossfader: self.crossfader,
            curve: self.curve,
            master_gain: self.master_target,
            master_peak: self.master_meter.value(),
            xruns: self.xruns,
        };
        self.io.snapshot_tx.write(snapshot);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 48_000;

    fn dc_track(id: u64, frames: usize, value: f32) -> Arc<TrackAudio> {
        Arc::new(TrackAudio::new(id, SR, vec![value; frames * 2]))
    }

    /// Render enough blocks for all smoothers to settle.
    fn settle(engine: &mut Engine) -> Vec<f32> {
        let mut out = vec![0.0; 256 * 2];
        for _ in 0..200 {
            engine.process(&mut out, 2);
        }
        out
    }

    fn deck_a(cmd: DeckCommand) -> Command {
        Command::Deck(DeckId::A, cmd)
    }

    fn mix(cmd: MixerCommand) -> Command {
        Command::Mixer(cmd)
    }

    #[test]
    fn plays_a_loaded_track_through_the_mixer() {
        let (mut engine, mut h) = engine_pair(SR, 256);
        h.load(DeckId::A, dc_track(1, SR as usize * 10, 0.5))
            .map_err(|_| ())
            .unwrap();
        h.send(deck_a(DeckCommand::Play));
        h.send(mix(MixerCommand::Crossfader(0.0)));
        let out = settle(&mut engine);
        // trim unity, fader 1.0, crossfader fully on A, master 1.0
        assert!((out[0] - 0.5).abs() < 1e-4, "{}", out[0]);
        assert!((out[1] - 0.5).abs() < 1e-4);
        let s = h.snapshot();
        assert_eq!(s.decks[0].track_id, Some(1));
        assert!(s.decks[0].playing);
        assert!((s.decks[0].peak[0] - 0.5).abs() < 1e-3);
    }

    #[test]
    fn centred_smooth_crossfader_gives_minus_three_db() {
        let (mut engine, mut h) = engine_pair(SR, 256);
        h.load(DeckId::A, dc_track(1, SR as usize * 10, 1.0))
            .map_err(|_| ())
            .unwrap();
        h.send(deck_a(DeckCommand::Play));
        let out = settle(&mut engine);
        assert!((out[0] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-3);
    }

    #[test]
    fn closed_channel_fader_is_silent_and_fades_without_jumps() {
        let (mut engine, mut h) = engine_pair(SR, 256);
        h.load(DeckId::A, dc_track(1, SR as usize * 10, 1.0))
            .map_err(|_| ())
            .unwrap();
        h.send(deck_a(DeckCommand::Play));
        settle(&mut engine);
        h.send(mix(MixerCommand::ChannelFader(DeckId::A, 0.0)));
        let mut out = vec![0.0; 256 * 2];
        let mut prev = f32::NAN;
        for _ in 0..200 {
            engine.process(&mut out, 2);
            for frame in out.as_chunks::<2>().0.iter() {
                if prev.is_finite() {
                    assert!((frame[0] - prev).abs() < 0.01, "zipper");
                }
                prev = frame[0];
            }
        }
        assert_eq!(out[0], 0.0);
    }

    #[test]
    fn track_end_stops_and_reports() {
        let (mut engine, mut h) = engine_pair(SR, 256);
        h.load(DeckId::A, dc_track(7, 300, 0.5))
            .map_err(|_| ())
            .unwrap();
        h.send(deck_a(DeckCommand::Play));
        let mut out = vec![0.0; 256 * 2];
        engine.process(&mut out, 2);
        engine.process(&mut out, 2);
        let s = h.snapshot();
        assert!(!s.decks[0].playing);
        assert_eq!(s.decks[0].position, 300);
        assert_eq!(
            h.next_event(),
            Some(EngineEvent::TrackLoaded {
                deck: DeckId::A,
                track_id: 7
            })
        );
        assert_eq!(
            h.next_event(),
            Some(EngineEvent::TrackEnded { deck: DeckId::A })
        );
    }

    #[test]
    fn replaced_track_comes_back_as_garbage() {
        let (mut engine, mut h) = engine_pair(SR, 256);
        let first = dc_track(1, 1000, 0.1);
        let weak = Arc::downgrade(&first);
        h.load(DeckId::A, first).map_err(|_| ()).unwrap();
        let mut out = vec![0.0; 64 * 2];
        engine.process(&mut out, 2);
        h.load(DeckId::A, dc_track(2, 1000, 0.1))
            .map_err(|_| ())
            .unwrap();
        engine.process(&mut out, 2);
        assert!(weak.upgrade().is_some(), "engine must not free it");
        assert_eq!(h.collect_garbage(), 1);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn unload_returns_track_and_empties_deck() {
        let (mut engine, mut h) = engine_pair(SR, 256);
        h.load(DeckId::B, dc_track(3, 1000, 0.1))
            .map_err(|_| ())
            .unwrap();
        h.send(Command::Deck(DeckId::B, DeckCommand::Unload));
        let mut out = vec![0.0; 64 * 2];
        engine.process(&mut out, 2);
        assert_eq!(h.snapshot().decks[1].track_id, None);
        assert_eq!(h.collect_garbage(), 1);
    }

    #[test]
    fn full_command_queue_is_counted_not_blocking() {
        let (_engine, mut h) = engine_pair(SR, 256);
        for _ in 0..COMMAND_QUEUE {
            assert!(h.send(deck_a(DeckCommand::Play)));
        }
        assert!(!h.send(deck_a(DeckCommand::Play)));
        assert_eq!(h.dropped_commands(), 1);
    }

    #[test]
    fn extra_device_channels_stay_silent() {
        let (mut engine, mut h) = engine_pair(SR, 256);
        h.load(DeckId::A, dc_track(1, SR as usize, 1.0))
            .map_err(|_| ())
            .unwrap();
        h.send(deck_a(DeckCommand::Play));
        let mut out = vec![9.0; 128 * 4];
        engine.process(&mut out, 4);
        for frame in out.as_chunks::<4>().0.iter() {
            assert_eq!(&frame[2..], &[0.0, 0.0]);
        }
    }

    #[test]
    fn device_buffers_larger_than_max_block_are_split() {
        let (mut engine, mut h) = engine_pair(SR, 64);
        h.load(DeckId::A, dc_track(1, SR as usize, 0.25))
            .map_err(|_| ())
            .unwrap();
        h.send(deck_a(DeckCommand::Play));
        let mut out = vec![0.0; 1000 * 2];
        engine.process(&mut out, 2);
        let s = h.snapshot();
        assert_eq!(s.frame_clock, 1000);
        assert_eq!(s.decks[0].position, 1000);
    }

    #[test]
    fn nan_parameters_are_neutralised() {
        let (mut engine, mut h) = engine_pair(SR, 256);
        h.send(mix(MixerCommand::MasterGain(f32::NAN)));
        let out = settle(&mut engine);
        assert!(out.iter().all(|v| v.is_finite()));
        assert_eq!(h.snapshot().master_gain, 0.0);
    }

    #[test]
    fn xruns_show_up_in_the_snapshot() {
        let (mut engine, mut h) = engine_pair(SR, 256);
        engine.note_xrun();
        engine.process(&mut [0.0; 4], 2);
        assert_eq!(h.snapshot().xruns, 1);
    }
}
