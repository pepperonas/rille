//! Runs the DDJ-200: hotplug, input → engine commands, engine state → LEDs, MIDI monitor.
//!
//! One thread owns everything controller related. CoreMIDI delivers bytes on its own thread;
//! they are forwarded here through a channel, so the session needs no lock.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use rille_core::{Command, DeckCommand, DeckId, MixerCommand, Snapshot};
use rille_engine::CommandSender;
use rille_midi::ddj200::{Control, ControllerAction, DeckLamps, Lamp, LampState, Scope};
use rille_midi::monitor::{Direction, Monitor, MonitorEntry, describe};
use rille_midi::port::{Connection, DDJ200_PORT, find_input};
use rille_midi::{ControllerSession, TakeoverKey};
use serde::Serialize;

use crate::audio_service::AudioService;

const TICK: Duration = Duration::from_millis(16);
const HOTPLUG_INTERVAL: Duration = Duration::from_secs(1);
/// Blink rate of LEDs that blink (paused deck, unset cue): 2 Hz.
const BLINK_PERIOD: Duration = Duration::from_millis(500);
/// A fader that sent nothing for this long is at rest: tempo changes now come from the engine.
const FADER_REST: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ControllerStatus {
    pub connected: bool,
    pub name: Option<String>,
    pub vinyl_mode: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorLine {
    /// Monotonic sequence number, stable key for the UI list.
    pub seq: u64,
    pub time_us: u64,
    pub outgoing: bool,
    pub hex: String,
    pub meaning: Option<String>,
}

impl From<MonitorEntry> for MonitorLine {
    fn from(e: MonitorEntry) -> MonitorLine {
        let hex = e.bytes[..usize::from(e.len)]
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(" ");
        MonitorLine {
            seq: e.seq,
            time_us: e.time_us,
            outgoing: e.direction == Direction::Out,
            hex,
            meaning: e.meaning,
        }
    }
}

pub enum ControllerEvent {
    Status(ControllerStatus),
    Monitor(Vec<MonitorLine>),
    /// Move the library selection by this many rows (shift + jog rim).
    LibraryScroll(i32),
    /// Load the selected library track onto this deck (shift + headphone cue).
    LibraryLoad(crate::dto::Deck),
}

/// Jog ticks per library row. An assumption until the hardware check: a slow turn of the rim
/// should step row by row, a quick spin skip a page.
pub const TICKS_PER_ROW: i32 = 4;

/// Bundles jog ticks into whole rows; the remainder carries over to the next tick, and a turn
/// in the other direction starts afresh.
#[derive(Debug, Default)]
pub struct ScrollAccumulator {
    ticks: i32,
}

impl ScrollAccumulator {
    pub fn push(&mut self, ticks: i32) -> i32 {
        if ticks.signum() != self.ticks.signum() {
            self.ticks = 0;
        }
        self.ticks += ticks;
        let rows = self.ticks / TICKS_PER_ROW;
        self.ticks -= rows * TICKS_PER_ROW;
        rows
    }
}

pub type ControllerNotify = Box<dyn Fn(ControllerEvent) + Send>;

enum Request {
    /// The app changed an absolute control; the hardware has to pick it up again.
    SoftwareChanged(TakeoverKey, f32),
    SetVinyl(bool),
    PadMode(DeckId, PadMode),
    Monitor(bool),
    Status(Sender<ControllerStatus>),
}

/// Handle used by Tauri commands.
pub struct ControllerService {
    requests: Sender<Request>,
}

impl ControllerService {
    pub fn start(audio: AudioService, notify: ControllerNotify) -> Option<ControllerService> {
        let sender = audio.take_controller_sender()?;
        let (requests, rx) = mpsc::channel();
        let spawned = thread::Builder::new()
            .name("rille-controller".into())
            .spawn(move || Worker::new(audio, sender, notify).run(rx));
        if let Err(e) = spawned {
            tracing::error!(%e, "could not start controller thread");
            return None;
        }
        Some(ControllerService { requests })
    }

    /// Report a change made in the app (not by the controller) to an absolute control.
    pub fn software_changed(&self, key: TakeoverKey, value: f32) {
        let _ = self.requests.send(Request::SoftwareChanged(key, value));
    }

    pub fn set_pad_mode(&self, deck: DeckId, mode: PadMode) {
        let _ = self.requests.send(Request::PadMode(deck, mode));
    }

    pub fn set_vinyl_mode(&self, on: bool) {
        let _ = self.requests.send(Request::SetVinyl(on));
    }

    pub fn set_monitor(&self, on: bool) {
        let _ = self.requests.send(Request::Monitor(on));
    }

    pub fn status(&self) -> ControllerStatus {
        let (tx, rx) = mpsc::channel();
        if self.requests.send(Request::Status(tx)).is_err() {
            return ControllerStatus::default();
        }
        rx.recv_timeout(Duration::from_millis(200))
            .unwrap_or_default()
    }
}

/// Lamps for one deck derived from the engine state, Pioneer style: Play blinks while a loaded
/// deck waits, Cue blinks when the deck is paused away from its cue point.
pub fn deck_lamps(snapshot: &Snapshot, deck: DeckId) -> DeckLamps {
    let d = &snapshot.decks[deck.index()];
    let loaded = d.track_id.is_some();
    let play = match (loaded, d.playing && !d.previewing) {
        (false, _) => Lamp::Off,
        (true, true) => Lamp::On,
        (true, false) => Lamp::Blink,
    };
    let cue = if !loaded {
        Lamp::Off
    } else if d.playing || d.position == d.cue {
        Lamp::On
    } else {
        Lamp::Blink
    };
    DeckLamps {
        play,
        cue,
        loaded: Lamp::from_bool(loaded),
        ..DeckLamps::default()
    }
}

/// The engine's values of every absolute control on the DDJ-200 (hardware orientation, 0..1).
pub fn software_values(s: &Snapshot) -> Vec<(rille_midi::TakeoverKey, f32)> {
    let mut v = vec![((Scope::Global, Control::Crossfader), s.crossfader)];
    for deck in DeckId::ALL {
        let i = deck.index();
        let d = Scope::Deck(deck);
        v.extend([
            ((d, Control::ChannelFader), s.channel_fader[i]),
            ((d, Control::EqLow), s.eq[i][0]),
            ((d, Control::EqMid), s.eq[i][1]),
            ((d, Control::EqHi), s.eq[i][2]),
            ((d, Control::ColorFx), s.filter[i]),
            (
                (d, Control::TempoFader),
                crate::dto::tempo_to_fader(s.decks[i].tempo),
            ),
        ]);
    }
    v
}

/// What the eight pads of a deck do. Chosen in the app (the DDJ-200 has no mode buttons).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PadMode {
    #[default]
    HotCue,
    BeatLoop,
    BeatJump,
}

/// Beat loop length per pad (pads 1–8).
pub const LOOP_BEATS: [f32; 8] = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0];
/// Beat jump per pad: top row back, bottom row forward, growing to the right.
pub const JUMP_BEATS: [f32; 8] = [-0.5, -1.0, -4.0, -16.0, 0.5, 1.0, 4.0, 16.0];

/// Engine commands for a pad press in `mode`. Shift: delete the hot cue, or leave the loop.
pub fn pad_commands(mode: PadMode, deck: DeckId, index: u8, shifted: bool) -> Vec<Command> {
    let i = usize::from(index);
    let cmd = match (mode, shifted) {
        (PadMode::HotCue, false) => DeckCommand::HotCue(index),
        (PadMode::HotCue, true) => DeckCommand::HotCueClear(index),
        (PadMode::BeatLoop, false) => match LOOP_BEATS.get(i) {
            Some(&beats) => DeckCommand::BeatLoop(beats),
            None => return Vec::new(),
        },
        (PadMode::BeatLoop, true) => DeckCommand::LoopExit,
        (PadMode::BeatJump, _) => match JUMP_BEATS.get(i) {
            Some(&beats) => DeckCommand::BeatJump(beats),
            None => return Vec::new(),
        },
    };
    vec![Command::Deck(deck, cmd)]
}

/// Pad lamps: empty pad off, set pad on, the active one blinks with the beat (on during the
/// first half of every beat; steady while paused).
pub fn pad_lamps(snapshot: &Snapshot, deck: DeckId, mode: PadMode) -> [Lamp; 8] {
    let d = &snapshot.decks[deck.index()];
    let mut pads = [Lamp::Off; 8];
    if d.track_id.is_none() {
        return pads;
    }
    let grid = d.grid.filter(|g| g.bpm > 0.0 && g.bpm.is_finite());
    let beat_on = match grid {
        Some(g) if d.playing => {
            let beat = rille_engine::beat::beat_at(&g, d.position as f64, snapshot.sample_rate);
            beat.rem_euclid(1.0) < 0.5
        }
        _ => true,
    };
    match mode {
        PadMode::HotCue => {
            for (pad, cue) in pads.iter_mut().zip(d.hot_cues) {
                *pad = Lamp::from_bool(cue.is_some());
            }
        }
        PadMode::BeatLoop => {
            if let (Some((start, end)), Some(g)) = (d.loop_range, grid) {
                let beats = (end - start) as f64
                    / rille_engine::beat::beat_frames(&g, snapshot.sample_rate);
                if let Some(i) = LOOP_BEATS
                    .iter()
                    .position(|&b| (f64::from(b) - beats).abs() < 0.01)
                {
                    pads[i] = Lamp::from_bool(beat_on);
                }
            }
        }
        PadMode::BeatJump => pads = [Lamp::from_bool(grid.is_some()); 8],
    }
    pads
}

pub fn lamp_state(snapshot: &Snapshot, modes: [PadMode; 2]) -> LampState {
    let t = snapshot.transition;
    let deck = |id: DeckId| DeckLamps {
        pads: pad_lamps(snapshot, id, modes[id.index()]),
        sync: Lamp::from_bool(snapshot.decks[id.index()].sync_lock),
        headphone_cue: Lamp::from_bool(snapshot.cue[id.index()]),
        ..deck_lamps(snapshot, id)
    };
    LampState {
        decks: [deck(DeckId::A), deck(DeckId::B)],
        // Lit while the effect runs, blinking while a cancelled effect's tail decays.
        transition_fx: match (t.deck, t.releasing) {
            (None, _) => Lamp::Off,
            (Some(_), false) => Lamp::On,
            (Some(_), true) => Lamp::Blink,
        },
        master_cue: Lamp::from_bool(snapshot.master_cue),
    }
}

/// Engine commands for a controller action. Actions whose features arrive in later milestones
/// (sync, pads, library, headphones) produce nothing yet.
pub fn commands_for(action: ControllerAction) -> Vec<Command> {
    use ControllerAction as A;
    let deck = |d, c| Command::Deck(d, c);
    match action {
        A::PlayPause(d) => vec![deck(d, DeckCommand::PlayPause)],
        A::Cue {
            deck: d,
            pressed: true,
        } => vec![deck(d, DeckCommand::CuePress)],
        A::Cue {
            deck: d,
            pressed: false,
        } => vec![deck(d, DeckCommand::CueRelease)],
        A::JumpToStart(d) => vec![deck(d, DeckCommand::JumpToStart)],
        A::SyncOnce(d) => vec![deck(d, DeckCommand::Sync)],
        A::SyncLockToggle(d) => vec![deck(d, DeckCommand::ToggleSyncLock)],
        A::Reverse { deck: d, on } => vec![deck(d, DeckCommand::Reverse(on))],
        A::CycleTempoRange(d) => vec![deck(d, DeckCommand::CycleTempoRange)],
        A::Tempo { deck: d, value } => vec![deck(d, DeckCommand::Tempo(value))],
        A::PitchBend { deck: d, ticks } => vec![deck(d, DeckCommand::Bend(ticks))],
        // The mapper only reports touches while vinyl mode is on, and releases them when it
        // goes off or the controller disconnects.
        A::ScratchTouch { deck: d, touching } => {
            vec![deck(d, DeckCommand::ScratchTouch(touching))]
        }
        A::Scratch { deck: d, ticks } => vec![deck(d, DeckCommand::Scratch(ticks))],
        A::Search { deck: d, ticks } => vec![deck(d, DeckCommand::Search(ticks))],
        A::ChannelFader { deck: d, value } => {
            vec![Command::Mixer(MixerCommand::ChannelFader(d, value))]
        }
        A::Crossfader(v) => vec![Command::Mixer(MixerCommand::Crossfader(v))],
        A::Eq {
            deck: d,
            band,
            value,
        } => {
            let band = match band {
                rille_midi::ddj200::EqBand::Hi => rille_core::EqBand::High,
                rille_midi::ddj200::EqBand::Mid => rille_core::EqBand::Mid,
                rille_midi::ddj200::EqBand::Low => rille_core::EqBand::Low,
            };
            vec![Command::Mixer(MixerCommand::Eq(d, band, value))]
        }
        A::Filter { deck: d, value } => vec![Command::Mixer(MixerCommand::Filter(d, value))],
        A::TransitionFx { pressed: true } => vec![Command::Mixer(MixerCommand::TransitionFx)],
        A::CycleTransitionFx => vec![Command::Mixer(MixerCommand::CycleTransitionFx)],
        A::HeadphoneCue(d) => vec![Command::Mixer(MixerCommand::ToggleHeadphoneCue(d))],
        A::MasterCue => vec![Command::Mixer(MixerCommand::ToggleMasterCue)],
        A::FaderStart {
            deck: d,
            play: true,
        } => vec![deck(d, DeckCommand::Play)],
        // Back to zero: return to the cue point and pause, like a CDJ back-cue.
        A::FaderStart {
            deck: d,
            play: false,
        } => vec![deck(d, DeckCommand::JumpToCue)],
        _ => Vec::new(),
    }
}

struct Worker {
    audio: AudioService,
    engine: CommandSender,
    notify: ControllerNotify,
    session: ControllerSession,
    connection: Option<Connection>,
    bytes_tx: Sender<(u64, Vec<u8>)>,
    bytes_rx: Receiver<(u64, Vec<u8>)>,
    monitor: Monitor,
    monitor_on: bool,
    started: Instant,
    scroll: ScrollAccumulator,
    pad_mode: [PadMode; 2],
    /// Tempo per deck as the engine last reported it, to notice changes made by sync.
    engine_tempo: [f32; 2],
    /// Last time the tempo fader of each deck sent a value.
    tempo_touched: [Option<Instant>; 2],
}

impl Worker {
    fn new(audio: AudioService, engine: CommandSender, notify: ControllerNotify) -> Worker {
        let (bytes_tx, bytes_rx) = mpsc::channel();
        Worker {
            audio,
            engine,
            notify,
            session: ControllerSession::new(),
            connection: None,
            bytes_tx,
            bytes_rx,
            monitor: Monitor::new(),
            monitor_on: false,
            started: Instant::now(),
            scroll: ScrollAccumulator::default(),
            pad_mode: [PadMode::default(); 2],
            engine_tempo: [0.0; 2],
            tempo_touched: [None; 2],
        }
    }

    /// Sync moves the tempo without the fader: the hardware fader must pick the new value up
    /// before it takes over again (soft takeover), or touching it would jump the tempo back.
    /// A change counts as the engine's own only while the fader rests: the snapshot lags
    /// behind a moving fader, and treating that lag as a foreign change would cut a sweep.
    fn follow_engine_tempo(&mut self, snapshot: &Snapshot) {
        for deck in DeckId::ALL {
            let i = deck.index();
            let tempo = snapshot.decks[i].tempo;
            if (tempo - self.engine_tempo[i]).abs() <= 1e-4 {
                continue;
            }
            // While the fader moves, wait: the change is probably the fader's own (lagging).
            // Once it rests, any remaining difference came from the engine and is reported.
            let resting = self.tempo_touched[i].is_none_or(|t| t.elapsed() > FADER_REST);
            if !resting {
                continue;
            }
            self.engine_tempo[i] = tempo;
            let from_fader = self
                .session
                .hardware_position((Scope::Deck(deck), Control::TempoFader))
                .is_some_and(|v| (v - crate::dto::tempo_to_fader(tempo)).abs() < 0.005);
            if !from_fader {
                self.session.software_changed(
                    (Scope::Deck(deck), Control::TempoFader),
                    crate::dto::tempo_to_fader(tempo),
                );
            }
        }
    }

    fn status(&self) -> ControllerStatus {
        ControllerStatus {
            connected: self.connection.is_some(),
            name: self.connection.as_ref().map(|c| c.name.clone()),
            vinyl_mode: self.session.vinyl_mode(),
        }
    }

    fn run(mut self, requests: Receiver<Request>) {
        tracing::info!("controller thread running, looking for {DDJ200_PORT}");
        let mut last_hotplug = Instant::now() - HOTPLUG_INTERVAL;
        loop {
            if last_hotplug.elapsed() >= HOTPLUG_INTERVAL {
                last_hotplug = Instant::now();
                self.hotplug();
            }
            while let Ok(request) = requests.try_recv() {
                match request {
                    Request::SetVinyl(on) => {
                        let mut out = Vec::new();
                        let released = self.session.set_vinyl_mode(on, &mut out);
                        self.send_actions(released);
                        self.send_all(&out);
                        (self.notify)(ControllerEvent::Status(self.status()));
                    }
                    Request::PadMode(deck, mode) => self.pad_mode[deck.index()] = mode,
                    Request::SoftwareChanged(key, value) => {
                        self.session.software_changed(key, value);
                    }
                    Request::Monitor(on) => {
                        if on && !self.monitor_on {
                            self.monitor = Monitor::new(); // no stale lines from last time
                        }
                        self.monitor_on = on;
                    }
                    Request::Status(reply) => {
                        let _ = reply.send(self.status());
                    }
                }
            }
            // Wait for controller input, but wake at least once per tick for LEDs.
            let deadline = Instant::now() + TICK;
            loop {
                let timeout = deadline.saturating_duration_since(Instant::now());
                match self.bytes_rx.recv_timeout(timeout) {
                    Ok((time_us, bytes)) => self.on_input(time_us, &bytes),
                    Err(RecvTimeoutError::Timeout) => break,
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
            self.tick();
        }
    }

    fn hotplug(&mut self) {
        let present = find_input(DDJ200_PORT).is_some();
        tracing::debug!(
            present,
            connected = self.connection.is_some(),
            "hotplug poll"
        );
        match (present, self.connection.is_some()) {
            (true, false) => self.connect(),
            (false, true) => {
                tracing::info!("controller disconnected");
                self.disconnected();
            }
            _ => {}
        }
    }

    fn connect(&mut self) {
        let tx = self.bytes_tx.clone();
        match Connection::open(DDJ200_PORT, move |time_us, bytes| {
            let _ = tx.send((time_us, bytes.to_vec()));
        }) {
            Ok(connection) => {
                tracing::info!(name = %connection.name, "controller connected");
                self.connection = Some(connection);
                let snapshot = self.audio.snapshot();
                let audible = snapshot.decks.iter().any(|d| d.playing);
                // Seed the software values first, then decide about pickup.
                self.seed_software(&snapshot);
                let mut out = Vec::new();
                let released = self.session.on_connect(!audible, &mut out);
                self.send_actions(released);
                self.send_all(&out);
                (self.notify)(ControllerEvent::Status(self.status()));
            }
            Err(e) => tracing::warn!(%e, "controller present but not connectable"),
        }
    }

    fn now_us(&self) -> u64 {
        self.started.elapsed().as_micros() as u64
    }

    fn on_input(&mut self, time_us: u64, bytes: &[u8]) {
        let _ = time_us;
        let (event, action) = self.session.on_input(bytes);
        if let Some(e) = &event
            && e.control == Control::TempoFader
            && let Scope::Deck(deck) = e.scope
        {
            self.tempo_touched[deck.index()] = Some(Instant::now());
        }
        if self.monitor_on {
            let meaning = event.as_ref().map(describe);
            self.monitor
                .push(self.now_us(), Direction::In, bytes, meaning);
        }
        if let Some(action) = action {
            self.send_actions([action]);
        }
    }

    fn send_actions(&mut self, actions: impl IntoIterator<Item = ControllerAction>) {
        for action in actions {
            match action {
                ControllerAction::LibraryScroll(ticks) => {
                    let rows = self.scroll.push(ticks);
                    if rows != 0 {
                        (self.notify)(ControllerEvent::LibraryScroll(rows));
                    }
                    continue;
                }
                ControllerAction::LoadSelected(deck) => {
                    (self.notify)(ControllerEvent::LibraryLoad(deck.into()));
                    continue;
                }
                ControllerAction::Pad {
                    deck,
                    index,
                    pressed: true,
                    shifted,
                } => {
                    for command in pad_commands(self.pad_mode[deck.index()], deck, index, shifted) {
                        self.engine.send(command);
                    }
                    continue;
                }
                _ => {}
            }
            for command in commands_for(action) {
                self.engine.send(command);
            }
        }
    }

    /// The controller is gone: drop the connection and release what it was holding (a touched
    /// platter or shift + play), otherwise the deck stays frozen or backwards.
    fn disconnected(&mut self) {
        self.connection = None;
        let released = self.session.release_held();
        self.send_actions(released);
        (self.notify)(ControllerEvent::Status(self.status()));
    }

    /// Engine values for the absolute controls, used when a controller connects. Not called
    /// continuously: the snapshot lags behind the hardware, and diffing it would make a fast
    /// fader move release itself.
    fn seed_software(&mut self, s: &Snapshot) {
        for (key, value) in software_values(s) {
            self.session.seed_software(key, value);
        }
    }

    fn tick(&mut self) {
        if self.connection.is_some() {
            let snapshot = self.audio.snapshot();
            let blink_on =
                (self.started.elapsed().as_millis() / BLINK_PERIOD.as_millis()).is_multiple_of(2);
            let mut out = Vec::new();
            self.follow_engine_tempo(&snapshot);
            self.session
                .update_leds(&lamp_state(&snapshot, self.pad_mode), blink_on, &mut out);
            self.send_all(&out);
        }
        if self.monitor_on {
            let lines: Vec<MonitorLine> = self
                .monitor
                .drain_new()
                .into_iter()
                .map(MonitorLine::from)
                .collect();
            if !lines.is_empty() {
                (self.notify)(ControllerEvent::Monitor(lines));
            }
        }
    }

    fn send_all(&mut self, messages: &[[u8; 3]]) {
        let now = self.now_us();
        let Some(connection) = self.connection.as_mut() else {
            return;
        };
        let mut failed = false;
        for msg in messages {
            if connection.send(msg) {
                if self.monitor_on {
                    self.monitor.push(now, Direction::Out, msg, None);
                }
            } else {
                failed = true;
            }
        }
        if failed {
            // Unplugged between polls: drop the connection; hotplug reconnects.
            self.disconnected();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rille_core::DeckSnapshot;

    fn snapshot_with(deck: DeckSnapshot) -> Snapshot {
        Snapshot {
            decks: [deck, DeckSnapshot::default()],
            ..Snapshot::default()
        }
    }

    #[test]
    fn empty_deck_is_dark() {
        let l = deck_lamps(&Snapshot::default(), DeckId::A);
        assert_eq!((l.play, l.cue, l.loaded), (Lamp::Off, Lamp::Off, Lamp::Off));
    }

    #[test]
    fn loaded_paused_deck_blinks_play() {
        let s = snapshot_with(DeckSnapshot {
            track_id: Some(1),
            frames: 100,
            ..Default::default()
        });
        let l = deck_lamps(&s, DeckId::A);
        assert_eq!((l.play, l.cue, l.loaded), (Lamp::Blink, Lamp::On, Lamp::On));
    }

    #[test]
    fn paused_away_from_cue_blinks_cue() {
        let s = snapshot_with(DeckSnapshot {
            track_id: Some(1),
            frames: 100,
            position: 50,
            ..Default::default()
        });
        assert_eq!(deck_lamps(&s, DeckId::A).cue, Lamp::Blink);
    }

    #[test]
    fn playing_deck_lights_play_and_cue() {
        let s = snapshot_with(DeckSnapshot {
            track_id: Some(1),
            frames: 100,
            position: 50,
            playing: true,
            ..Default::default()
        });
        let l = deck_lamps(&s, DeckId::A);
        assert_eq!((l.play, l.cue), (Lamp::On, Lamp::On));
    }

    #[test]
    fn preview_does_not_light_play() {
        let s = snapshot_with(DeckSnapshot {
            track_id: Some(1),
            frames: 100,
            playing: true,
            previewing: true,
            ..Default::default()
        });
        assert_eq!(deck_lamps(&s, DeckId::A).play, Lamp::Blink);
    }

    #[test]
    fn every_absolute_control_is_seeded_from_the_engine() {
        let mut s = Snapshot::default();
        s.eq[1] = [0.1, 0.2, 0.3];
        s.filter[0] = 0.9;
        s.decks[1].tempo = -1.0;
        let v = software_values(&s);
        assert!(v.contains(&((Scope::Deck(DeckId::B), Control::EqHi), 0.3)));
        assert!(v.contains(&((Scope::Deck(DeckId::A), Control::ColorFx), 0.9)));
        assert!(v.contains(&((Scope::Deck(DeckId::B), Control::TempoFader), 0.0)));
        assert!(v.contains(&((Scope::Deck(DeckId::A), Control::TempoFader), 0.5)));
        assert_eq!(v.len(), 1 + 2 * 6);
    }

    #[test]
    fn transition_lamp_follows_the_effect() {
        let mut s = Snapshot::default();
        assert_eq!(
            lamp_state(&s, [PadMode::HotCue; 2]).transition_fx,
            Lamp::Off
        );
        s.transition.deck = Some(DeckId::A);
        assert_eq!(lamp_state(&s, [PadMode::HotCue; 2]).transition_fx, Lamp::On);
        s.transition.releasing = true;
        assert_eq!(
            lamp_state(&s, [PadMode::HotCue; 2]).transition_fx,
            Lamp::Blink
        );
    }

    #[test]
    fn actions_become_engine_commands() {
        use ControllerAction as A;
        assert_eq!(
            commands_for(A::Cue {
                deck: DeckId::B,
                pressed: true
            }),
            [Command::Deck(DeckId::B, DeckCommand::CuePress)]
        );
        assert_eq!(
            commands_for(A::FaderStart {
                deck: DeckId::A,
                play: false
            }),
            [Command::Deck(DeckId::A, DeckCommand::JumpToCue)]
        );
        assert_eq!(
            commands_for(A::Crossfader(0.25)),
            [Command::Mixer(MixerCommand::Crossfader(0.25))]
        );
        assert!(
            commands_for(A::LibraryScroll(1)).is_empty(),
            "library actions are events for the frontend, not engine commands"
        );
        assert_eq!(
            commands_for(A::Eq {
                deck: DeckId::B,
                band: rille_midi::ddj200::EqBand::Hi,
                value: 0.25
            }),
            [Command::Mixer(MixerCommand::Eq(
                DeckId::B,
                rille_core::EqBand::High,
                0.25
            ))]
        );
        assert_eq!(
            commands_for(A::TransitionFx { pressed: true }),
            [Command::Mixer(MixerCommand::TransitionFx)]
        );
        assert!(
            commands_for(A::TransitionFx { pressed: false }).is_empty(),
            "press, not release"
        );
        let d = DeckId::A;
        let cases = [
            (
                A::Tempo {
                    deck: d,
                    value: 0.5,
                },
                DeckCommand::Tempo(0.5),
            ),
            (A::CycleTempoRange(d), DeckCommand::CycleTempoRange),
            (A::PitchBend { deck: d, ticks: 3 }, DeckCommand::Bend(3)),
            (A::Scratch { deck: d, ticks: -4 }, DeckCommand::Scratch(-4)),
            (A::Search { deck: d, ticks: 2 }, DeckCommand::Search(2)),
            (A::Reverse { deck: d, on: true }, DeckCommand::Reverse(true)),
            (
                A::ScratchTouch {
                    deck: d,
                    touching: false,
                },
                DeckCommand::ScratchTouch(false),
            ),
        ];
        for (action, cmd) in cases {
            assert_eq!(commands_for(action), [Command::Deck(d, cmd)], "{action:?}");
        }
    }

    #[test]
    fn monitor_lines_are_hex() {
        let line = MonitorLine::from(MonitorEntry {
            seq: 1,
            time_us: 5,
            direction: Direction::Out,
            bytes: [0x9F, 0x01, 0x7F],
            len: 3,
            meaning: None,
        });
        assert_eq!((line.hex.as_str(), line.outgoing), ("9F 01 7F", true));
    }

    #[test]
    fn jog_ticks_are_bundled_into_rows() {
        let mut acc = ScrollAccumulator::default();
        let rows: Vec<i32> = [1, 1, 1, 1, 1, 5, -1, -3, 2]
            .into_iter()
            .map(|t| acc.push(t))
            .collect();
        // 4 ticks make a row; leftovers carry; reversing drops the leftover.
        assert_eq!(rows, [0, 0, 0, 1, 0, 1, 0, -1, 0]);
        let mut fast = ScrollAccumulator::default();
        assert_eq!(fast.push(TICKS_PER_ROW * 5 + 1), 5);
    }

    fn looping(beats: f64) -> Snapshot {
        let mut s = Snapshot {
            sample_rate: 48_000,
            ..Snapshot::default()
        };
        let d = &mut s.decks[0];
        d.track_id = Some(1);
        d.grid = Some(rille_core::BeatGrid {
            bpm: 120.0,
            first_beat: 0.0,
        });
        d.loop_range = Some((48_000, 48_000 + (beats * 24_000.0) as u64));
        s
    }

    #[test]
    fn pads_follow_the_pad_mode() {
        let d = |c| Command::Deck(DeckId::B, c);
        assert_eq!(
            pad_commands(PadMode::HotCue, DeckId::B, 2, false),
            [d(DeckCommand::HotCue(2))]
        );
        assert_eq!(
            pad_commands(PadMode::HotCue, DeckId::B, 2, true),
            [d(DeckCommand::HotCueClear(2))]
        );
        assert_eq!(
            pad_commands(PadMode::BeatLoop, DeckId::B, 0, false),
            [d(DeckCommand::BeatLoop(0.25))]
        );
        assert_eq!(
            pad_commands(PadMode::BeatLoop, DeckId::B, 7, false),
            [d(DeckCommand::BeatLoop(32.0))]
        );
        assert_eq!(
            pad_commands(PadMode::BeatLoop, DeckId::B, 5, true),
            [d(DeckCommand::LoopExit)]
        );
        assert_eq!(
            pad_commands(PadMode::BeatJump, DeckId::B, 0, false),
            [d(DeckCommand::BeatJump(-0.5))]
        );
        assert_eq!(
            pad_commands(PadMode::BeatJump, DeckId::B, 7, false),
            [d(DeckCommand::BeatJump(16.0))]
        );
        assert!(pad_commands(PadMode::BeatLoop, DeckId::B, 8, false).is_empty());
    }

    #[test]
    fn pad_lamps_show_set_cues_and_blink_the_active_loop_with_the_beat() {
        let mut s = looping(4.0);
        s.decks[0].hot_cues[1] = Some(10);
        let cues = pad_lamps(&s, DeckId::A, PadMode::HotCue);
        assert_eq!(cues[1], Lamp::On);
        assert_eq!(cues[0], Lamp::Off);

        // Paused: the active loop pad is steadily on.
        let loops = pad_lamps(&s, DeckId::A, PadMode::BeatLoop);
        assert_eq!(loops[4], Lamp::On, "4 beats is pad 5");
        assert_eq!(loops.iter().filter(|l| **l == Lamp::On).count(), 1);
        // Playing: on in the first half of a beat, off in the second.
        s.decks[0].playing = true;
        s.decks[0].position = 48_000 + 3_000;
        assert_eq!(pad_lamps(&s, DeckId::A, PadMode::BeatLoop)[4], Lamp::On);
        s.decks[0].position = 48_000 + 15_000;
        assert_eq!(pad_lamps(&s, DeckId::A, PadMode::BeatLoop)[4], Lamp::Off);

        assert_eq!(pad_lamps(&s, DeckId::A, PadMode::BeatJump), [Lamp::On; 8]);
        s.decks[0].grid = None;
        assert_eq!(pad_lamps(&s, DeckId::A, PadMode::BeatJump), [Lamp::Off; 8]);
        assert_eq!(
            pad_lamps(&s, DeckId::B, PadMode::HotCue),
            [Lamp::Off; 8],
            "empty deck"
        );
    }

    #[test]
    fn sync_buttons_and_lamp() {
        assert_eq!(
            commands_for(ControllerAction::SyncOnce(DeckId::A)),
            [Command::Deck(DeckId::A, DeckCommand::Sync)]
        );
        assert_eq!(
            commands_for(ControllerAction::SyncLockToggle(DeckId::B)),
            [Command::Deck(DeckId::B, DeckCommand::ToggleSyncLock)]
        );
        let mut s = looping(1.0);
        assert_eq!(
            lamp_state(&s, [PadMode::HotCue; 2]).decks[0].sync,
            Lamp::Off
        );
        s.decks[0].sync_lock = true;
        assert_eq!(lamp_state(&s, [PadMode::HotCue; 2]).decks[0].sync, Lamp::On);
    }

    #[test]
    fn headphone_buttons_toggle_and_light_up() {
        assert_eq!(
            commands_for(ControllerAction::HeadphoneCue(DeckId::B)),
            [Command::Mixer(MixerCommand::ToggleHeadphoneCue(DeckId::B))]
        );
        assert_eq!(
            commands_for(ControllerAction::MasterCue),
            [Command::Mixer(MixerCommand::ToggleMasterCue)]
        );
        let s = Snapshot {
            cue: [false, true],
            master_cue: true,
            ..Snapshot::default()
        };
        let lamps = lamp_state(&s, [PadMode::HotCue; 2]);
        assert_eq!(
            (
                lamps.decks[0].headphone_cue,
                lamps.decks[1].headphone_cue,
                lamps.master_cue
            ),
            (Lamp::Off, Lamp::On, Lamp::On)
        );
    }
}
