//! The realtime engine: decks + mixer, rendered block by block.
//!
//! [`Engine::process`] is the only entry point from the audio thread. It must stay free of
//! allocation, locks, logging and I/O. Everything it needs is allocated in [`engine_pair`].

use std::sync::Arc;

use rille_core::{
    BeatGrid, Command, CrossfaderCurve, CueRoute, DeckCommand, DeckId, DeckSnapshot, HOT_CUES,
    MixerCommand, Snapshot, TrackAudio,
};
use rtrb::{Consumer, Producer, RingBuffer};
use triple_buffer::{Input, Output};

use crate::beat;
use crate::curves::{channel_gain, crossfader_gains, trim_gain};
use crate::dsp::eq::Eq;
use crate::dsp::filter::BipolarFilter;
use crate::dsp::transition::{Outcome, Transition};
use crate::meter::PeakMeter;
use crate::player::Player;
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
    /// Known from the library already (analysed before); later through `SetGrid`.
    pub grid: Option<BeatGrid>,
    /// Saved hot cues, in engine frames.
    pub hot_cues: [Option<u64>; HOT_CUES],
}

impl TrackLoad {
    pub fn new(deck: DeckId, track: Arc<TrackAudio>) -> TrackLoad {
        TrackLoad {
            deck,
            track,
            grid: None,
            hot_cues: [None; HOT_CUES],
        }
    }
}

/// Seconds sync lock takes to pull a phase error in (before the nudge limit applies).
const SYNC_CORRECTION_S: f64 = 1.0;

/// Things the engine reports that must not get lost (unlike the snapshot, which is overwritten).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineEvent {
    TrackLoaded {
        deck: DeckId,
        track_id: u64,
    },
    TrackEnded {
        deck: DeckId,
    },
    /// A transition effect ran to its end and paused the deck.
    TransitionFinished {
        deck: DeckId,
    },
}

/// Engine-side ends of all queues.
pub struct EngineIo {
    ui_rx: Consumer<Command>,
    midi_rx: Consumer<Command>,
    track_rx: Consumer<TrackLoad>,
    garbage_tx: Producer<Arc<TrackAudio>>,
    events_tx: Producer<EngineEvent>,
    snapshot_tx: Input<Snapshot>,
    /// Headphone signal for a second device (read by `cue::DriftReader`).
    cue_tx: Producer<f32>,
}

/// Sending end of a command queue for exactly one producer thread (e.g. the controller).
pub struct CommandSender {
    tx: Producer<Command>,
    dropped: u32,
}

impl CommandSender {
    /// Queue a command. Returns `false` (and counts it) when the queue is full; never blocks.
    pub fn send(&mut self, command: Command) -> bool {
        let ok = self.tx.push(command).is_ok();
        if !ok {
            self.dropped = self.dropped.saturating_add(1);
        }
        ok
    }

    pub fn dropped(&self) -> u32 {
        self.dropped
    }
}

/// Control-side ends of all queues. Lives outside the audio thread.
pub struct EngineHandle {
    ui_tx: Producer<Command>,
    midi_tx: Option<CommandSender>,
    track_tx: Producer<TrackLoad>,
    garbage_rx: Consumer<Arc<TrackAudio>>,
    events_rx: Consumer<EngineEvent>,
    snapshot_rx: Output<Snapshot>,
    cue_rx: Option<Consumer<f32>>,
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

    /// The second command queue, for the controller thread. Can be taken once.
    pub fn take_midi_sender(&mut self) -> Option<CommandSender> {
        self.midi_tx.take()
    }

    /// Hand a decoded track to a deck. Gives the track back if the queue is full.
    pub fn load(&mut self, deck: DeckId, track: Arc<TrackAudio>) -> Result<(), Arc<TrackAudio>> {
        self.load_track(TrackLoad::new(deck, track))
            .map_err(|load| load.track)
    }

    /// Hand a decoded track with its grid and hot cues to a deck. Gives it back if the queue
    /// is full.
    // The load travels by value through the lock-free queue; boxing it would mean freeing the
    // box on the audio thread.
    #[allow(clippy::result_large_err)]
    pub fn load_track(&mut self, load: TrackLoad) -> Result<(), TrackLoad> {
        self.track_tx.push(load).map_err(|e| match e {
            rtrb::PushError::Full(load) => load,
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

    /// The reading end of the headphone ring, for the second-device stream. Can be taken once.
    pub fn take_cue_reader(&mut self) -> Option<Consumer<f32>> {
        self.cue_rx.take()
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
    let (cue_tx, cue_rx) = crate::cue::cue_ring();
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
        cue_tx,
    };
    let handle = EngineHandle {
        ui_tx,
        midi_tx: Some(CommandSender {
            tx: midi_tx,
            dropped: 0,
        }),
        track_tx,
        garbage_rx,
        events_rx,
        snapshot_rx,
        cue_rx: Some(cue_rx),
        dropped_commands: 0,
    };
    (Engine::new(sample_rate, max_block, io), handle)
}

/// Length of the declick ramps: stopping, starting and jumping fade over this time instead of
/// cutting the waveform (a hard cut is an audible click).
pub const DECLICK_MS: f32 = 4.0;

/// The old playback position, still sounding while it fades out after a stop or jump.
#[derive(Debug, Clone, Copy)]
struct Tail {
    position: u64,
    remaining: u32,
}

struct Deck {
    track: Option<Arc<TrackAudio>>,
    transport: Transport,
    /// Rendered stereo frames for the current block (interleaved), sized for `max_block`.
    buffer: Vec<f32>,
    gain: Smoother,
    meter: PeakMeter,
    declick_frames: u32,
    tail: Option<Tail>,
    /// The tail was armed since the last render: it still holds the position that was actually
    /// heard, so a second jump in the same block must not replace it.
    tail_unheard: bool,
    /// Frames left of the fade-in after a start or jump.
    fade_in: u32,
    eq: Eq,
    filter: BipolarFilter,
    player: Player,
    grid: Option<BeatGrid>,
    hot_cues: [Option<u64>; HOT_CUES],
    /// `[start, end)` in frames.
    loop_range: Option<(u64, u64)>,
    sync_lock: bool,
    /// When sync lock was switched on (block count); with both decks locked, the later one
    /// follows the other.
    locked_at: u64,
}

impl Deck {
    fn new(sample_rate: u32, max_block: usize) -> Deck {
        Deck {
            track: None,
            transport: Transport::new(0),
            buffer: vec![0.0; max_block * 2],
            gain: Smoother::new(sample_rate, LEVEL_SMOOTHING_MS, 0.0),
            meter: PeakMeter::new(sample_rate),
            declick_frames: ((DECLICK_MS / 1000.0) * sample_rate as f32).max(1.0) as u32,
            tail: None,
            tail_unheard: false,
            fade_in: 0,
            eq: Eq::new(sample_rate),
            filter: BipolarFilter::new(sample_rate),
            player: Player::new(sample_rate, max_block),
            grid: None,
            hot_cues: [None; HOT_CUES],
            loop_range: None,
            sync_lock: false,
            locked_at: 0,
        }
    }

    fn grid(&self) -> Option<BeatGrid> {
        beat::usable(&self.grid)
    }

    /// Jump within the render (loop, sync): the transport moves, the player follows and the
    /// declick crossfades from what was just heard.
    fn jump(&mut self, to: f64) {
        let before = self.transport;
        let to = to.clamp(0.0, self.transport.frames as f64);
        if (to - self.player.playhead()).abs() < 0.5 {
            return;
        }
        self.transport.position = to as u64;
        // The exact position, fraction included: a loop must repeat to the sub-frame.
        self.player.jump_to(to);
        self.declick(before);
    }

    /// Frames that can be rendered before the playhead reaches the loop end (at least 1, at
    /// most `max`). The rate is an estimate; a short overshoot is carried into the loop start.
    fn until_loop_end(&self, max: usize) -> usize {
        let Some((_, end)) = self.loop_range else {
            return max;
        };
        if !self.transport.playing || self.player.reverse() {
            return max;
        }
        let ahead = end as f64 - self.player.playhead();
        if ahead <= 0.0 {
            return 1;
        }
        let rate = f64::from(self.player.rate().max(self.player.tempo_rate())).max(0.05);
        ((ahead / rate).ceil() as usize).clamp(1, max)
    }

    /// The playhead crossed the loop end while playing forward (it was at `from` before the
    /// part): back to the start, keeping the overshoot. Only a crossing loops back – a jump
    /// that lands beyond the loop has already left it (see `apply_deck`).
    fn wrap_loop(&mut self, from: f64) {
        let Some((start, end)) = self.loop_range else {
            return;
        };
        let pos = self.player.playhead();
        let end_f = end as f64;
        if !self.transport.playing || self.player.reverse() || from >= end_f || pos < end_f {
            return;
        }
        let len = (end - start).max(1) as f64;
        let over = (pos - end as f64).rem_euclid(len);
        let to = start as f64 + over;
        // With keylock the stretcher has been fed the next pass already: nothing restarts.
        if self.player.loop_wrap(to) {
            self.transport.position = to as u64;
            return;
        }
        // Audio was rendered since the last jump: the tail must start from what was heard now.
        self.tail_unheard = false;
        self.jump(to);
    }

    /// EQ and filter, in place on the rendered block.
    fn shape(&mut self, frames: usize) {
        let block = &mut self.buffer[..frames * 2];
        self.eq.process(block);
        self.filter.process(block);
    }

    /// Compare transport state before and after a command and arm the declick ramps for any
    /// discontinuity: a stop or jump keeps the old position sounding while it fades out, a start
    /// or jump fades the new position in. A jump therefore becomes a short crossfade.
    fn declick(&mut self, before: Transport) {
        let after = self.transport;
        let jumped = after.position != before.position;
        // With keylock the player crossfades its own stretcher out and the new audio in: a tail
        // from the raw track would not match what was just heard, and a fade-in on top would
        // pull the outgoing stretcher audio to zero at once.
        if self.player.owns_fade_out() {
            return;
        }
        if before.playing && (!after.playing || jumped) && !self.tail_unheard {
            self.tail = Some(Tail {
                position: before.position,
                remaining: self.declick_frames,
            });
            self.tail_unheard = true;
        }
        if after.playing && (!before.playing || jumped) {
            self.fade_in = self.declick_frames;
        }
    }

    /// Render `frames` frames into `self.buffer`. Returns `true` when the track ended. A loop
    /// splits the block at its end, so the jump back lands on the exact frame.
    fn render(&mut self, frames: usize) -> bool {
        self.tail_unheard = false;
        // Taken out for the render (a move, no reference count change) and put back below.
        let Some(track) = self.track.take() else {
            self.buffer[..frames * 2].fill(0.0);
            self.tail = None;
            return false;
        };
        // The length can still change while the track loads (an MP3 estimate is corrected
        // once decoding finishes).
        self.transport.frames = track.frames() as u64;
        self.player.set_loop(self.loop_range);
        let mut done = 0;
        let mut ended = false;
        while done < frames {
            let n = self.until_loop_end(frames - done);
            let from = self.player.playhead();
            ended |= self.render_part(&track, done, n);
            done += n;
            self.wrap_loop(from);
        }
        self.track = Some(track);
        ended
    }

    /// Render `frames` frames at `offset` in the block.
    fn render_part(&mut self, track: &TrackAudio, offset: usize, frames: usize) -> bool {
        let out = &mut self.buffer[offset * 2..(offset + frames) * 2];
        let playing = self.transport.playing;
        let hit = self.player.render(track, playing, out);
        let length = self.declick_frames as f32;
        for frame in out.as_chunks_mut::<2>().0 {
            if self.fade_in > 0 {
                self.fade_in -= 1;
                let gain = 1.0 - self.fade_in as f32 / length;
                frame[0] *= gain;
                frame[1] *= gain;
            }
            if let Some(tail) = &mut self.tail {
                let gain = tail.remaining as f32 / length;
                let [l, r] = track.frame(tail.position as usize);
                frame[0] += l * gain;
                frame[1] += r * gain;
                tail.position += 1;
                tail.remaining -= 1;
                if tail.remaining == 0 {
                    self.tail = None;
                }
            }
        }
        self.transport.position =
            (self.player.playhead().floor() as u64).min(self.transport.frames);
        if playing && hit.end {
            self.transport.pause();
            return true;
        }
        if playing && hit.start && self.player.reverse() {
            self.transport.pause();
        }
        false
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
    transition: Transition,
    frame_clock: u64,
    xruns: u32,
    cue: [bool; 2],
    /// Per channel: trim while on the cue bus, else 0 (smoothed: no click when switching).
    cue_gain: [Smoother; 2],
    master_cue: bool,
    master_cue_gain: Smoother,
    cue_mix: f32,
    cue_mix_gain: Smoother,
    cue_level: f32,
    cue_level_gain: Smoother,
    cue_meter: PeakMeter,
    cue_route: CueRoute,
}

impl Engine {
    fn initial_snapshot() -> Snapshot {
        Snapshot {
            channel_fader: [1.0; 2],
            trim: [0.5; 2],
            crossfader: 0.5,
            master_gain: 1.0,
            eq: [[0.5; 3]; 2],
            filter: [0.5; 2],
            cue_mix: 0.5,
            cue_level: 0.8,
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
            transition: Transition::new(sample_rate),
            cue: [false; 2],
            cue_gain: [
                Smoother::new(sample_rate, LEVEL_SMOOTHING_MS, 0.0),
                Smoother::new(sample_rate, LEVEL_SMOOTHING_MS, 0.0),
            ],
            master_cue: false,
            master_cue_gain: Smoother::new(sample_rate, LEVEL_SMOOTHING_MS, 0.0),
            cue_mix: defaults.cue_mix,
            cue_mix_gain: Smoother::new(sample_rate, LEVEL_SMOOTHING_MS, defaults.cue_mix),
            cue_level: defaults.cue_level,
            cue_level_gain: Smoother::new(
                sample_rate,
                LEVEL_SMOOTHING_MS,
                channel_gain(defaults.cue_level),
            ),
            cue_meter: PeakMeter::new(sample_rate),
            cue_route: CueRoute::Off,
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
    /// master; channels 3/4 carry the headphones when routed there (`CueRoute::Channels34`),
    /// all further channels stay silent.
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
        self.update_sync_lock();

        for (index, deck) in self.decks.iter_mut().enumerate() {
            if deck.render(frames)
                && let Some(id) = DeckId::from_index(index)
            {
                let _ = self.io.events_tx.push(EngineEvent::TrackEnded { deck: id });
            }
            deck.shape(frames);
        }

        let fx_deck = self.transition.active();
        let [a, b] = &mut self.decks;
        let mut deck_peak = [[0.0f32; 2]; 2];
        let mut master_peak = [0.0f32; 2];
        let mut cue_peak = [0.0f32; 2];
        let route = self.cue_route;
        let [cga, cgb] = &mut self.cue_gain;
        for i in 0..frames {
            let ga = a.gain.tick();
            let gb = b.gain.tick();
            let gm = self.master_gain.tick();
            // Headphone bus: pre-fader, so read before channel gain and effects.
            let (ca, cb) = (cga.tick(), cgb.tick());
            let cue_l = a.buffer[2 * i] * ca + b.buffer[2 * i] * cb;
            let cue_r = a.buffer[2 * i + 1] * ca + b.buffer[2 * i + 1] * cb;
            let (mut al, mut ar) = (a.buffer[2 * i] * ga, a.buffer[2 * i + 1] * ga);
            let (mut bl, mut br) = (b.buffer[2 * i] * gb, b.buffer[2 * i + 1] * gb);
            match fx_deck {
                Some(DeckId::A) => [al, ar] = self.transition.process([al, ar]),
                Some(DeckId::B) => [bl, br] = self.transition.process([bl, br]),
                None => {}
            }
            let l = (al + bl) * gm;
            let r = (ar + br) * gm;
            deck_peak[0] = [deck_peak[0][0].max(al.abs()), deck_peak[0][1].max(ar.abs())];
            deck_peak[1] = [deck_peak[1][0].max(bl.abs()), deck_peak[1][1].max(br.abs())];
            master_peak = [master_peak[0].max(l.abs()), master_peak[1].max(r.abs())];
            let mc = self.master_cue_gain.tick();
            let mix = self.cue_mix_gain.tick();
            let level = self.cue_level_gain.tick();
            let hp_l = level * ((1.0 - mix) * (cue_l + mc * l) + mix * l);
            let hp_r = level * ((1.0 - mix) * (cue_r + mc * r) + mix * r);
            cue_peak = [cue_peak[0].max(hp_l.abs()), cue_peak[1].max(hp_r.abs())];
            let frame = &mut out[i * channels..(i + 1) * channels];
            frame[0] = l;
            if channels > 1 {
                frame[1] = r;
                frame[2..].fill(0.0);
            }
            match route {
                CueRoute::Channels34 if channels >= 4 => {
                    frame[2] = hp_l;
                    frame[3] = hp_r;
                }
                // Full ring (no reader, or it stalls): dropped, never blocking.
                CueRoute::Ring => {
                    let _ = crate::cue::push_frame(&mut self.io.cue_tx, hp_l, hp_r);
                }
                _ => {}
            }
        }
        a.meter.update(deck_peak[0], frames);
        b.meter.update(deck_peak[1], frames);
        self.master_meter.update(master_peak, frames);
        self.cue_meter.update(cue_peak, frames);
        self.frame_clock += frames as u64;

        if let Outcome::Finished { deck, pause: true } = self.transition.end_of_block() {
            // The effect already took the deck to silence: stop it without a declick tail.
            self.decks[deck.index()].transport.pause();
            let _ = self
                .io
                .events_tx
                .push(EngineEvent::TransitionFinished { deck });
        }
    }

    /// The deck a transition effect should take out: the playing deck that is loudest in the
    /// master right now (channel fader × crossfader × trim).
    fn transition_target(&self) -> Option<DeckId> {
        let (xa, xb) = crossfader_gains(self.crossfader, self.curve);
        let xf = [xa, xb];
        DeckId::ALL
            .into_iter()
            .filter(|d| self.decks[d.index()].transport.playing)
            .map(|d| {
                let i = d.index();
                (
                    d,
                    trim_gain(self.trim[i]) * channel_gain(self.channel_fader[i]) * xf[i],
                )
            })
            .filter(|(_, level)| *level > 0.0)
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(d, _)| d)
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
        // New material on the deck: a running transition on it lets go (no muting, no pause).
        self.transition.release_deck(load.deck);
        let deck = &mut self.decks[load.deck.index()];
        let track_id = load.track.id;
        deck.transport = Transport::new(load.track.frames() as u64);
        deck.player.reset_for_track();
        deck.grid = load.grid;
        deck.hot_cues = load.hot_cues;
        deck.loop_range = None;
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
        match cmd {
            DeckCommand::Sync => return self.sync_once(id),
            DeckCommand::SyncLock(on) => return self.set_sync_lock(id, on),
            DeckCommand::ToggleSyncLock => {
                let on = !self.decks[id.index()].sync_lock;
                return self.set_sync_lock(id, on);
            }
            _ => {}
        }
        if cmd == DeckCommand::Unload {
            self.transition.release_deck(id);
            let deck = &mut self.decks[id.index()];
            deck.transport = Transport::new(0);
            deck.player.reset_for_track();
            deck.grid = None;
            deck.hot_cues = [None; HOT_CUES];
            deck.loop_range = None;
            deck.sync_lock = false;
            deck.player.set_sync_nudge(0.0);
            let old = deck.track.take();
            self.release(old);
            return;
        }
        let deck = &mut self.decks[id.index()];
        let before = deck.transport;
        // Rebound for each arm that needs them; `deck` stays usable for the beat commands.
        let (t, p) = (&mut deck.transport, &mut deck.player);
        match cmd {
            DeckCommand::PlayPause => t.play_pause(),
            DeckCommand::Play => t.play(),
            DeckCommand::Pause => t.pause(),
            DeckCommand::CuePress => t.cue_press(),
            DeckCommand::CueRelease => t.cue_release(),
            DeckCommand::JumpToStart => t.jump_to_start(),
            DeckCommand::JumpToCue => t.jump_to_cue(),
            DeckCommand::Seek { frame } => t.seek(frame),
            DeckCommand::Unload => {}
            DeckCommand::Tempo(position) => p.set_tempo(position),
            DeckCommand::TempoRange(r) => p.set_range(r),
            DeckCommand::CycleTempoRange => p.cycle_range(),
            DeckCommand::Keylock(on) => p.set_keylock(on),
            DeckCommand::Bend(ticks) => p.bend(ticks),
            DeckCommand::BendHold(dir) => p.bend_hold(dir),
            DeckCommand::ScratchTouch(on) => p.scratch_touch(on),
            DeckCommand::Scratch(ticks) => p.scratch(ticks),
            DeckCommand::Search(ticks) => {
                p.search(ticks, t.frames);
                t.position = (p.playhead() as u64).min(t.frames);
            }
            DeckCommand::Reverse(on) => p.set_reverse(on),
            DeckCommand::Sync | DeckCommand::SyncLock(_) | DeckCommand::ToggleSyncLock => {}
            DeckCommand::SetGrid(grid) => deck.grid = grid,
            DeckCommand::BeatLoop(beats) => {
                deck.loop_range = beat_loop(
                    deck.grid(),
                    deck.loop_range,
                    beats,
                    deck.player.playhead(),
                    deck.transport.frames,
                    self.sample_rate,
                );
                // Shortened loop already behind the playhead: back to the same place within it.
                if let Some((start, end)) = deck.loop_range {
                    let pos = deck.player.playhead();
                    if pos >= end as f64 {
                        let len = (end - start).max(1) as f64;
                        deck.transport.position =
                            (start as f64 + (pos - start as f64) % len) as u64;
                    }
                }
            }
            DeckCommand::LoopExit => deck.loop_range = None,
            DeckCommand::BeatJump(beats) => {
                if let Some(g) = deck.grid()
                    && beats.is_finite()
                {
                    let delta = f64::from(beats) * beat::beat_frames(&g, self.sample_rate);
                    let last = deck.transport.frames.saturating_sub(1) as f64;
                    match deck.loop_range {
                        // A running loop moves along, keeping its length; a jump that would
                        // push it off the track is not made.
                        Some((start, end)) => {
                            let (s, e) = (start as f64 + delta, end as f64 + delta);
                            if s >= 0.0 && e <= last {
                                deck.loop_range = Some((s as u64, e as u64));
                                deck.transport.position =
                                    (deck.player.playhead() + delta).clamp(0.0, last) as u64;
                            }
                        }
                        None => {
                            deck.transport.position =
                                (deck.player.playhead() + delta).clamp(0.0, last) as u64;
                        }
                    }
                }
            }
            DeckCommand::HotCue(slot) => {
                let quantized =
                    beat::quantize_cue(deck.grid(), deck.player.playhead(), self.sample_rate);
                let frames = deck.transport.frames;
                if let Some(cue) = deck.hot_cues.get_mut(usize::from(slot)) {
                    match *cue {
                        Some(at) => deck.transport.position = at.min(frames),
                        None => *cue = Some((quantized as u64).min(frames)),
                    }
                }
            }
            DeckCommand::HotCueClear(slot) => {
                if let Some(cue) = deck.hot_cues.get_mut(usize::from(slot)) {
                    *cue = None;
                }
            }
        }
        // A jump made by the transport (cue, seek, jump to start) moves the playhead.
        if deck.transport.position != before.position && !matches!(cmd, DeckCommand::Search(_)) {
            deck.player.jump_to(deck.transport.position as f64);
        }
        // Jumping out of a loop (cue, hot cue, seek, search) leaves it, like on a CDJ.
        if let Some((start, end)) = deck.loop_range
            && deck.transport.position != before.position
            && !(start..end).contains(&deck.transport.position)
        {
            deck.loop_range = None;
        }
        let started = deck.transport.playing && !before.playing;
        deck.declick(before);
        // Starting the deck again (play, cue preview) means the DJ wants to hear it: a running
        // transition on it lets go instead of muting and later pausing the new playback.
        if started {
            self.transition.release_deck(id);
        }
    }

    /// Sync source for `id`: the other deck, if it has a track and a grid.
    fn sync_source(&self, id: DeckId) -> Option<(DeckId, BeatGrid)> {
        let other = id.other();
        let d = &self.decks[other.index()];
        d.track.as_ref()?;
        Some((other, d.grid()?))
    }

    /// Tempo (and range) for `id` to follow `source`. `None` without grids or out of reach.
    fn follow(&self, id: DeckId, source: DeckId, source_grid: BeatGrid) -> Option<beat::Follow> {
        let own = self.decks[id.index()].grid()?;
        let target_bpm =
            source_grid.bpm * f64::from(self.decks[source.index()].player.tempo_rate());
        beat::follow(own.bpm, target_bpm)
    }

    /// Phase error of `id` against `source` in own beats (positive = ahead).
    fn phase_error(&self, id: DeckId, source: DeckId, follow: beat::Follow) -> Option<f64> {
        let own = &self.decks[id.index()];
        let src = &self.decks[source.index()];
        let own_beat = beat::beat_at(&own.grid()?, own.player.playhead(), self.sample_rate);
        let src_beat = beat::beat_at(&src.grid()?, src.player.playhead(), self.sample_rate);
        Some(beat::phase_error(
            own_beat,
            src_beat,
            follow.beats_per_target_beat,
        ))
    }

    /// Set `id`'s tempo fader (widening the range if needed) to play at `rate`.
    fn set_rate(&mut self, id: DeckId, rate: f64) {
        let p = &mut self.decks[id.index()].player;
        if let Some((range, position)) = beat::tempo_for_rate(rate, p.range())
            && ((position - p.tempo()).abs() > 1e-6 || range != p.range())
        {
            p.set_range(range);
            p.set_tempo(position);
        }
    }

    /// SYNC pressed: match the other deck's tempo; if it plays, also its phase (one short
    /// crossfaded jump).
    fn sync_once(&mut self, id: DeckId) {
        let Some((source, grid)) = self.sync_source(id) else {
            return;
        };
        let Some(follow) = self.follow(id, source, grid) else {
            return;
        };
        self.set_rate(id, follow.rate);
        // Phase only between two playing decks: a paused deck stays on its cue (the target
        // moves on anyway until Play); sync lock pulls the phase in once both play.
        if !self.decks[source.index()].transport.playing
            || !self.decks[id.index()].transport.playing
        {
            return;
        }
        let Some(error) = self.phase_error(id, source, follow) else {
            return;
        };
        let deck = &mut self.decks[id.index()];
        let Some(own) = deck.grid() else { return };
        let to = deck.player.playhead() - error * beat::beat_frames(&own, self.sample_rate);
        let last = deck.transport.frames.saturating_sub(1) as f64;
        deck.jump(to.clamp(0.0, last));
    }

    fn set_sync_lock(&mut self, id: DeckId, on: bool) {
        let deck = &mut self.decks[id.index()];
        if on && !deck.sync_lock {
            deck.locked_at = self.frame_clock + 1;
        }
        deck.sync_lock = on;
        if !on {
            deck.player.set_sync_nudge(0.0);
        } else {
            self.sync_once(id);
        }
    }

    /// Once per block: a locked deck follows the other deck's tempo and pulls its phase in
    /// through a small rate correction (never a jump). With both locked, the later one follows.
    fn update_sync_lock(&mut self) {
        for id in DeckId::ALL {
            let (me, other) = (&self.decks[id.index()], &self.decks[id.other().index()]);
            let follows = me.sync_lock
                && me.track.is_some()
                && other.track.is_some()
                && other.transport.playing
                && !(other.sync_lock && other.locked_at > me.locked_at);
            let nudge = if follows { self.lock_step(id) } else { 0.0 };
            self.decks[id.index()].player.set_sync_nudge(nudge);
        }
    }

    /// Tempo follow plus the phase correction for one locked deck.
    fn lock_step(&mut self, id: DeckId) -> f32 {
        let Some((source, grid)) = self.sync_source(id) else {
            return 0.0;
        };
        let Some(follow) = self.follow(id, source, grid) else {
            return 0.0;
        };
        self.set_rate(id, follow.rate);
        let deck = &self.decks[id.index()];
        if !deck.transport.playing || deck.player.hands_on() {
            return 0.0;
        }
        let Some(error) = self.phase_error(id, source, follow) else {
            return 0.0;
        };
        let Some(own) = deck.grid() else { return 0.0 };
        // Seconds of error, made up within SYNC_CORRECTION_S: ahead → slower.
        let seconds = error * 60.0 / (own.bpm * follow.rate);
        (-seconds / SYNC_CORRECTION_S) as f32
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
            MixerCommand::Eq(id, band, v) => self.decks[id.index()].eq.set_knob(band, v),
            MixerCommand::EqKill(id, band, kill) => self.decks[id.index()].eq.set_kill(band, kill),
            MixerCommand::Filter(id, v) => self.decks[id.index()].filter.set_position(v),
            MixerCommand::TransitionFx => {
                // A running effect is cancelled on its own deck; otherwise pick the target.
                if let Some(deck) = self
                    .transition
                    .active()
                    .or_else(|| self.transition_target())
                {
                    self.transition.trigger(deck);
                }
            }
            MixerCommand::CycleTransitionFx => self.transition.cycle(),
            MixerCommand::HeadphoneCue(d, on) => self.cue[d.index()] = on,
            MixerCommand::MasterCue(on) => self.master_cue = on,
            MixerCommand::ToggleHeadphoneCue(d) => self.cue[d.index()] ^= true,
            MixerCommand::ToggleMasterCue => self.master_cue ^= true,
            MixerCommand::CueMix(v) => self.cue_mix = unit(v),
            MixerCommand::CueLevel(v) => self.cue_level = unit(v),
            MixerCommand::CueRoute(route) => self.cue_route = route,
        }
        self.update_gain_targets();
    }

    fn update_gain_targets(&mut self) {
        let (xa, xb) = crossfader_gains(self.crossfader, self.curve);
        let xf = [xa, xb];
        for (i, deck) in self.decks.iter_mut().enumerate() {
            deck.gain
                .set_target(trim_gain(self.trim[i]) * channel_gain(self.channel_fader[i]) * xf[i]);
            // The cue bus is taken before channel fader and crossfader, after trim.
            let on = self.cue.get(i).copied().unwrap_or(false);
            if let Some(g) = self.cue_gain.get_mut(i) {
                g.set_target(if on { trim_gain(self.trim[i]) } else { 0.0 });
            }
        }
        self.master_cue_gain
            .set_target(if self.master_cue { 1.0 } else { 0.0 });
        self.cue_mix_gain.set_target(self.cue_mix);
        self.cue_level_gain.set_target(channel_gain(self.cue_level));
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
            tempo: d.player.tempo(),
            tempo_range: d.player.range(),
            keylock: d.player.keylock(),
            rate: d.player.rate(),
            reverse: d.player.reverse(),
            scratching: d.player.scratching(),
            grid: d.grid,
            hot_cues: d.hot_cues,
            loop_range: d.loop_range,
            sync_lock: d.sync_lock,
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
            eq: [self.decks[0].eq.knobs(), self.decks[1].eq.knobs()],
            eq_kill: [self.decks[0].eq.kills(), self.decks[1].eq.kills()],
            filter: [
                self.decks[0].filter.position(),
                self.decks[1].filter.position(),
            ],
            transition: self.transition.snapshot(),
            xruns: self.xruns,
            cue: self.cue,
            master_cue: self.master_cue,
            cue_mix: self.cue_mix,
            cue_level: self.cue_level,
            cue_peak: self.cue_meter.value(),
            cue_route: self.cue_route,
        };
        self.io.snapshot_tx.write(snapshot);
    }
}

/// The loop after BEAT LOOP `beats`: a new loop from the beat at or before the playhead (on a
/// grid of the loop length, up to one beat), the same length again leaves it, another length
/// resizes it from its start. Without a grid there is no beat loop. Ends at the track end.
fn beat_loop(
    grid: Option<BeatGrid>,
    current: Option<(u64, u64)>,
    beats: f32,
    playhead: f64,
    frames: u64,
    sample_rate: u32,
) -> Option<(u64, u64)> {
    let g = grid?;
    if !(beats.is_finite() && beats > 0.0) {
        return current;
    }
    let len = f64::from(beats) * beat::beat_frames(&g, sample_rate);
    if let Some((start, end)) = current {
        if ((end - start) as f64 - len).abs() < 1.0 {
            return None;
        }
        let end = (start as f64 + len).round() as u64;
        // Too long for the rest of the track: the loop stays as it is.
        if end >= frames {
            return current;
        }
        return (end > start).then_some((start, end));
    }
    // On the beat (step) at or before the playhead – or the next one, if it is less than ⅛
    // step away: a press just before the downbeat means the downbeat.
    let step = f64::from(beats.min(1.0));
    let at = beat::beat_at(&g, playhead, sample_rate) / step;
    let n = if at - at.floor() > 0.875 {
        at.ceil()
    } else {
        at.floor()
    };
    let start = beat::frame_of_beat(&g, n * step, sample_rate).max(0.0);
    let end = (start + len).round() as u64;
    let start = start.round() as u64;
    // The loop end must lie before the last frame, or the track would end instead of looping.
    (end > start && end < frames).then_some((start, end))
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
    fn controller_queue_is_drained_too() {
        let (mut engine, mut h) = engine_pair(SR, 256);
        let mut midi = h.take_midi_sender().unwrap();
        assert!(h.take_midi_sender().is_none(), "only one producer");
        midi.send(mix(MixerCommand::Crossfader(0.0)));
        engine.process(&mut [0.0; 4], 2);
        assert_eq!(h.snapshot().crossfader, 0.0);
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

    /// Largest jump between consecutive output samples (left channel) over `blocks` blocks.
    fn max_step(engine: &mut Engine, blocks: usize, prev: &mut f32) -> f32 {
        let mut out = vec![0.0; 64 * 2];
        let mut worst = 0.0f32;
        for _ in 0..blocks {
            engine.process(&mut out, 2);
            for frame in out.as_chunks::<2>().0 {
                worst = worst.max((frame[0] - *prev).abs());
                *prev = frame[0];
            }
        }
        worst
    }

    #[test]
    fn pause_fades_out_instead_of_clicking() {
        let (mut engine, mut h) = engine_pair(SR, 64);
        h.load(DeckId::A, dc_track(1, SR as usize * 10, 1.0))
            .map_err(|_| ())
            .unwrap();
        h.send(mix(MixerCommand::Crossfader(0.0)));
        h.send(deck_a(DeckCommand::Play));
        let mut prev = 0.0;
        max_step(&mut engine, 50, &mut prev); // fade in, settle
        h.send(deck_a(DeckCommand::Pause));
        let step = max_step(&mut engine, 20, &mut prev);
        assert!(step < 0.01, "step {step}");
        // The EQ filters ring out for a moment; inaudible is what matters.
        assert!(prev.abs() < 1e-6, "silent afterwards: {prev}");
    }

    #[test]
    fn play_fades_in() {
        let (mut engine, mut h) = engine_pair(SR, 64);
        h.load(DeckId::A, dc_track(1, SR as usize * 10, 1.0))
            .map_err(|_| ())
            .unwrap();
        h.send(mix(MixerCommand::Crossfader(0.0)));
        let mut prev = 0.0;
        max_step(&mut engine, 10, &mut prev);
        h.send(deck_a(DeckCommand::Play));
        assert!(max_step(&mut engine, 20, &mut prev) < 0.01);
    }

    #[test]
    fn cue_jump_while_playing_crossfades() {
        // First half +1, second half -1: a hard jump would step by 2.
        let half = SR as usize;
        let mut samples = vec![1.0f32; half * 2];
        samples.extend(vec![-1.0f32; half * 2]);
        let (mut engine, mut h) = engine_pair(SR, 64);
        h.load(DeckId::A, Arc::new(TrackAudio::new(1, SR, samples)))
            .map_err(|_| ())
            .unwrap();
        h.send(mix(MixerCommand::Crossfader(0.0)));
        h.send(deck_a(DeckCommand::Seek {
            frame: half as u64 + 1000,
        }));
        h.send(deck_a(DeckCommand::CuePress)); // set cue in the -1 region
        h.send(deck_a(DeckCommand::Seek { frame: 0 }));
        h.send(deck_a(DeckCommand::Play));
        let mut prev = 0.0;
        max_step(&mut engine, 50, &mut prev);
        h.send(deck_a(DeckCommand::CuePress)); // playing: jump to cue (-1) and pause
        let step = max_step(&mut engine, 20, &mut prev);
        assert!(step < 0.02, "step {step}");
    }

    #[test]
    fn seek_while_playing_crossfades() {
        let half = SR as usize;
        let mut samples = vec![1.0f32; half * 2];
        samples.extend(vec![-1.0f32; half * 2]);
        let (mut engine, mut h) = engine_pair(SR, 64);
        h.load(DeckId::A, Arc::new(TrackAudio::new(1, SR, samples)))
            .map_err(|_| ())
            .unwrap();
        h.send(mix(MixerCommand::Crossfader(0.0)));
        h.send(deck_a(DeckCommand::Play));
        let mut prev = 0.0;
        max_step(&mut engine, 50, &mut prev);
        h.send(deck_a(DeckCommand::Seek {
            frame: half as u64 + 10,
        }));
        let step = max_step(&mut engine, 20, &mut prev);
        assert!(step < 0.02, "step {step}");
        assert!(prev < -0.99, "now playing the second half");
    }

    #[test]
    fn eq_kill_and_filter_come_from_commands() {
        use rille_core::EqBand;
        let (mut engine, mut h) = engine_pair(SR, 256);
        h.load(DeckId::A, dc_track(1, SR as usize * 10, 0.5))
            .map_err(|_| ())
            .unwrap();
        h.send(mix(MixerCommand::Crossfader(0.0)));
        h.send(deck_a(DeckCommand::Play));
        let out = settle(&mut engine);
        assert!(out[0] > 0.4, "DC passes the neutral EQ");
        h.send(mix(MixerCommand::EqKill(DeckId::A, EqBand::Low, true)));
        h.send(mix(MixerCommand::Filter(DeckId::A, 0.2)));
        let out = settle(&mut engine);
        assert!(
            out[0].abs() < 0.01,
            "DC sits in the low band: killed ({})",
            out[0]
        );
        let s = h.snapshot();
        assert!(s.eq_kill[0][0]);
        assert_eq!(s.filter[0], 0.2);
        assert_eq!(s.eq[1], [0.5; 3], "deck B untouched");
    }

    fn sine_track(id: u64, seconds: usize) -> Arc<TrackAudio> {
        let samples = (0..SR as usize * seconds)
            .flat_map(|i| {
                let v = 0.5 * (i as f32 * 440.0 * std::f32::consts::TAU / SR as f32).sin();
                [v, v]
            })
            .collect();
        Arc::new(TrackAudio::new(id, SR, samples))
    }

    #[test]
    fn transition_fx_takes_out_the_loudest_playing_deck() {
        let (mut engine, mut h) = engine_pair(SR, 256);
        h.load(DeckId::A, sine_track(1, 30))
            .map_err(|_| ())
            .unwrap();
        h.load(DeckId::B, sine_track(2, 30))
            .map_err(|_| ())
            .unwrap();
        h.send(deck_a(DeckCommand::Play));
        h.send(Command::Deck(DeckId::B, DeckCommand::Play));
        h.send(mix(MixerCommand::Crossfader(0.8))); // B is louder
        settle(&mut engine);
        h.send(mix(MixerCommand::TransitionFx));
        let mut out = vec![0.0; 256 * 2];
        engine.process(&mut out, 2);
        assert_eq!(h.snapshot().transition.deck, Some(DeckId::B));
        for _ in 0..(12 * SR as usize / 256) {
            engine.process(&mut out, 2);
        }
        let s = h.snapshot();
        assert!(!s.decks[1].playing, "B paused after the echoes");
        assert!(s.decks[0].playing, "A keeps playing");
        assert_eq!(s.transition.deck, None);
        let mut finished = false;
        while let Some(e) = h.next_event() {
            finished |= e == EngineEvent::TransitionFinished { deck: DeckId::B };
        }
        assert!(finished);
    }

    #[test]
    fn transition_fx_needs_a_playing_deck() {
        let (mut engine, mut h) = engine_pair(SR, 256);
        h.load(DeckId::A, sine_track(1, 5)).map_err(|_| ()).unwrap();
        h.send(mix(MixerCommand::TransitionFx));
        engine.process(&mut [0.0; 512], 2);
        assert_eq!(h.snapshot().transition.deck, None);
    }

    #[test]
    fn pressing_again_cancels_and_keeps_playing() {
        let (mut engine, mut h) = engine_pair(SR, 256);
        h.load(DeckId::A, sine_track(1, 30))
            .map_err(|_| ())
            .unwrap();
        h.send(deck_a(DeckCommand::Play));
        h.send(mix(MixerCommand::TransitionFx));
        settle(&mut engine);
        h.send(mix(MixerCommand::TransitionFx));
        let mut out = vec![0.0; 256 * 2];
        for _ in 0..(8 * SR as usize / 256) {
            engine.process(&mut out, 2);
        }
        let s = h.snapshot();
        assert!(s.decks[0].playing);
        assert_eq!(s.transition.deck, None);
        assert!(s.master_peak[0] > 0.3, "dry signal back");
    }

    #[test]
    fn loading_a_new_track_during_echo_out_keeps_it_audible_and_playing() {
        let (mut engine, mut h) = engine_pair(SR, 256);
        h.load(DeckId::A, sine_track(1, 30))
            .map_err(|_| ())
            .unwrap();
        h.send(deck_a(DeckCommand::Play));
        settle(&mut engine);
        h.send(mix(MixerCommand::TransitionFx));
        settle(&mut engine); // dry faded out
        h.load(DeckId::A, sine_track(2, 30))
            .map_err(|_| ())
            .unwrap();
        h.send(deck_a(DeckCommand::Play));
        let mut out = vec![0.0; 256 * 2];
        for _ in 0..(10 * SR as usize / 256) {
            engine.process(&mut out, 2);
        }
        let s = h.snapshot();
        assert_eq!(s.decks[0].track_id, Some(2));
        assert!(
            s.decks[0].playing,
            "the new track was not paused by the old effect"
        );
        assert!(s.decks[0].peak[0] > 0.3, "and it is audible");
        assert_eq!(s.transition.deck, None);
    }

    #[test]
    fn restarting_a_deck_during_echo_out_cancels_it() {
        let (mut engine, mut h) = engine_pair(SR, 256);
        h.load(DeckId::A, sine_track(1, 30))
            .map_err(|_| ())
            .unwrap();
        h.send(deck_a(DeckCommand::Play));
        settle(&mut engine);
        h.send(mix(MixerCommand::TransitionFx));
        engine.process(&mut [0.0; 512], 2);
        h.send(deck_a(DeckCommand::Pause));
        h.send(deck_a(DeckCommand::Play));
        engine.process(&mut [0.0; 512], 2);
        assert!(h.snapshot().transition.releasing);
    }

    #[test]
    fn tempo_keylock_and_reverse_reach_the_deck() {
        use rille_core::TempoRange;
        let (mut engine, mut h) = engine_pair(SR, 256);
        h.load(DeckId::A, sine_track(1, 30))
            .map_err(|_| ())
            .unwrap();
        h.send(deck_a(DeckCommand::TempoRange(TempoRange::Sixteen)));
        h.send(deck_a(DeckCommand::Tempo(-1.0)));
        h.send(deck_a(DeckCommand::Keylock(true)));
        h.send(deck_a(DeckCommand::Play));
        settle(&mut engine);
        let s = h.snapshot().decks[0];
        assert_eq!(
            (s.tempo, s.tempo_range, s.keylock),
            (-1.0, TempoRange::Sixteen, true)
        );
        assert!((s.rate - 0.84).abs() < 0.001, "rate {}", s.rate);
        let before = s.position;
        h.send(deck_a(DeckCommand::Reverse(true)));
        settle(&mut engine);
        let s = h.snapshot().decks[0];
        assert!(s.reverse && s.rate < 0.0);
        assert!(s.position < before, "moved backwards");
    }

    /// Largest sample-to-sample step of the left channel over `blocks` blocks, including the
    /// step from the last sample before them.
    fn largest_step(engine: &mut Engine, blocks: usize, prev: &mut f32) -> f32 {
        let mut out = vec![0.0; 256 * 2];
        let mut step = 0.0f32;
        for _ in 0..blocks {
            engine.process(&mut out, 2);
            for f in out.as_chunks::<2>().0 {
                step = step.max((f[0] - *prev).abs());
                *prev = f[0];
            }
        }
        step
    }

    /// Three unrelated partials: unlike a pure sine, a phase vocoder does not reproduce this
    /// waveform sample for sample, so a hard switch between stretched and raw audio shows up.
    fn rich_track(seconds: usize) -> Arc<TrackAudio> {
        let samples = (0..SR as usize * seconds)
            .flat_map(|i| {
                let t = i as f32 / SR as f32;
                let v = [(110.0, 0.3), (523.0, 0.2), (1871.0, 0.1)]
                    .iter()
                    .map(|(f, a)| a * (t * f * std::f32::consts::TAU).sin())
                    .sum::<f32>();
                [v, v]
            })
            .collect();
        Arc::new(TrackAudio::new(1, SR, samples))
    }

    #[test]
    fn a_track_plays_while_it_is_still_loading() {
        let sr = SR as usize;
        // Expected 10 s (an MP3 estimate), only 1 s decoded so far.
        let track = Arc::new(TrackAudio::streaming(1, SR, 10 * sr, 11 * sr));
        let tone: Vec<f32> = (0..sr)
            .flat_map(|i| [0.5 * (i as f32 * 440.0 * std::f32::consts::TAU / SR as f32).sin(); 2])
            .collect();
        track.write(0, &tone);
        let (mut engine, mut h) = engine_pair(SR, 256);
        h.load(DeckId::A, track.clone()).map_err(|_| ()).unwrap();
        h.send(mix(MixerCommand::Crossfader(0.0)));
        h.send(deck_a(DeckCommand::Play));
        let mut out = vec![0.0; 256 * 2];
        for _ in 0..40 {
            engine.process(&mut out, 2); // ~0.2 s, inside the published second
        }
        assert!(out.iter().any(|v| v.abs() > 0.1), "published audio plays");
        assert_eq!(h.snapshot().decks[0].frames, 10 * SR as u64);
        // Decoding ends early: the real length replaces the estimate, and playback stops there.
        let more: Vec<f32> = tone.iter().map(|v| v * 0.5).collect();
        track.write(sr, &more);
        track.finish(2 * sr);
        let mut out = vec![0.0; 256 * 2];
        for _ in 0..(3 * sr / 256) {
            engine.process(&mut out, 2);
        }
        let s = h.snapshot().decks[0];
        assert_eq!(s.frames, 2 * SR as u64);
        assert!(!s.playing, "stopped at the real end");
    }

    #[test]
    fn two_jumps_in_one_block_still_crossfade_from_what_was_heard() {
        let (mut engine, mut h) = engine_pair(SR, 256);
        h.load(DeckId::A, rich_track(30)).map_err(|_| ()).unwrap();
        h.send(mix(MixerCommand::Crossfader(0.0)));
        h.send(deck_a(DeckCommand::Play));
        settle(&mut engine);
        let mut prev = 0.0;
        largest_step(&mut engine, 1, &mut prev);
        let steady = largest_step(&mut engine, 8, &mut prev);
        // 1.85 s: not a whole number of periods of the test partials, so the jump lands on a
        // different phase (a 2 s jump would land on the same waveform and hide any click).
        h.send(deck_a(DeckCommand::Search(37)));
        h.send(deck_a(DeckCommand::Search(37)));
        let across = largest_step(&mut engine, 8, &mut prev);
        assert!(across <= steady * 1.5, "step {across} vs steady {steady}");
    }

    #[test]
    fn leaving_keylock_does_not_click() {
        let leave: [(&str, &[DeckCommand]); 5] = [
            ("pause", &[DeckCommand::Pause]),
            // 10.37 s: off the test partials' common period, so the target has another phase.
            (
                "cue jump",
                &[DeckCommand::Seek {
                    frame: 10 * SR as u64 + 17_760,
                }],
            ),
            ("reverse", &[DeckCommand::Reverse(true)]),
            ("scratch touch", &[DeckCommand::ScratchTouch(true)]),
            ("keylock off", &[DeckCommand::Keylock(false)]),
        ];
        for (name, cmds) in leave {
            let (mut engine, mut h) = engine_pair(SR, 256);
            h.load(DeckId::A, rich_track(30)).map_err(|_| ()).unwrap();
            h.send(mix(MixerCommand::Crossfader(0.0)));
            h.send(deck_a(DeckCommand::Keylock(true)));
            h.send(deck_a(DeckCommand::Tempo(0.5)));
            h.send(deck_a(DeckCommand::Play));
            settle(&mut engine);
            let mut prev = 0.0;
            largest_step(&mut engine, 1, &mut prev); // only to seed `prev`
            let steady = largest_step(&mut engine, 8, &mut prev);
            for c in cmds {
                h.send(deck_a(*c));
            }
            let across = largest_step(&mut engine, 8, &mut prev);
            assert!(
                across <= steady * 1.5,
                "{name}: step {across} vs steady {steady}"
            );
        }
    }

    #[test]
    fn cue_jump_with_keylock_keeps_sounding() {
        let (mut engine, mut h) = engine_pair(SR, 256);
        h.load(DeckId::A, sine_track(1, 30))
            .map_err(|_| ())
            .unwrap();
        h.send(mix(MixerCommand::Crossfader(0.0)));
        h.send(deck_a(DeckCommand::Keylock(true)));
        h.send(deck_a(DeckCommand::Tempo(0.5)));
        h.send(deck_a(DeckCommand::Play));
        settle(&mut engine);
        h.send(deck_a(DeckCommand::Seek {
            frame: 10 * u64::from(SR),
        }));
        let mut out = vec![0.0; 256 * 2];
        let mut quiet_blocks = 0;
        for _ in 0..20 {
            engine.process(&mut out, 2);
            if out.iter().all(|v| v.abs() < 0.05) {
                quiet_blocks += 1;
            }
        }
        assert_eq!(quiet_blocks, 0, "no gap after the jump");
    }

    #[test]
    fn keylock_loops_do_not_click_at_the_seam_or_when_changed() {
        // Each case: commands sent once the loop runs, then several seconds of passes. 0.37
        // beats is off the test partials' common period, so every seam joins different phases.
        let cases: [(&str, &[DeckCommand]); 4] = [
            ("passes", &[]),
            ("halved", &[DeckCommand::BeatLoop(0.25)]),
            ("left", &[DeckCommand::LoopExit]),
            ("keylock off", &[DeckCommand::Keylock(false)]),
        ];
        for (name, cmds) in cases {
            let (mut engine, mut h) = engine_pair(SR, 256);
            let load = TrackLoad {
                grid: Some(BeatGrid {
                    bpm: 120.0,
                    first_beat: 0.0,
                }),
                ..TrackLoad::new(DeckId::A, rich_track(30))
            };
            assert!(h.load_track(load).is_ok());
            h.send(mix(MixerCommand::Crossfader(0.0)));
            h.send(deck_a(DeckCommand::Keylock(true)));
            h.send(deck_a(DeckCommand::Tempo(0.5)));
            h.send(deck_a(DeckCommand::Seek {
                frame: 5 * u64::from(SR) + 7_000,
            }));
            h.send(deck_a(DeckCommand::Play));
            settle(&mut engine);
            let mut prev = 0.0;
            largest_step(&mut engine, 1, &mut prev);
            let steady = largest_step(&mut engine, 8, &mut prev);
            h.send(deck_a(DeckCommand::BeatLoop(0.37)));
            let looping = largest_step(&mut engine, 100, &mut prev);
            assert!(
                looping <= steady * 1.5,
                "{name}: step {looping} vs steady {steady} while looping"
            );
            for c in cmds {
                h.send(deck_a(*c));
            }
            let after = largest_step(&mut engine, 200, &mut prev);
            assert!(
                after <= steady * 1.5,
                "{name}: step {after} vs steady {steady}"
            );
        }
    }

    #[test]
    fn xruns_show_up_in_the_snapshot() {
        let (mut engine, mut h) = engine_pair(SR, 256);
        engine.note_xrun();
        engine.process(&mut [0.0; 4], 2);
        assert_eq!(h.snapshot().xruns, 1);
    }
}
