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
}

pub type ControllerNotify = Box<dyn Fn(ControllerEvent) + Send>;

enum Request {
    /// The app changed an absolute control; the hardware has to pick it up again.
    SoftwareChanged(TakeoverKey, f32),
    SetVinyl(bool),
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

pub fn lamp_state(snapshot: &Snapshot) -> LampState {
    let t = snapshot.transition;
    LampState {
        decks: [
            deck_lamps(snapshot, DeckId::A),
            deck_lamps(snapshot, DeckId::B),
        ],
        // Lit while the effect runs, blinking while a cancelled effect's tail decays.
        transition_fx: match (t.deck, t.releasing) {
            (None, _) => Lamp::Off,
            (Some(_), false) => Lamp::On,
            (Some(_), true) => Lamp::Blink,
        },
        ..LampState::default()
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
        A::Reverse { deck: d, on } => vec![deck(d, DeckCommand::Reverse(on))],
        A::CycleTempoRange(d) => vec![deck(d, DeckCommand::CycleTempoRange)],
        A::Tempo { deck: d, value } => vec![deck(d, DeckCommand::Tempo(value))],
        A::PitchBend { deck: d, ticks } => vec![deck(d, DeckCommand::Bend(ticks))],
        // The mapping only reports touches while vinyl mode is on.
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
                        self.session.set_vinyl_mode(on, &mut out);
                        self.send_all(&out);
                        (self.notify)(ControllerEvent::Status(self.status()));
                    }
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
                self.connection = None;
                (self.notify)(ControllerEvent::Status(self.status()));
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
                self.session.on_connect(!audible, &mut out);
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
        if self.monitor_on {
            let meaning = event.as_ref().map(describe);
            self.monitor
                .push(self.now_us(), Direction::In, bytes, meaning);
        }
        if let Some(action) = action {
            for command in commands_for(action) {
                self.engine.send(command);
            }
        }
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
            self.session
                .update_leds(&lamp_state(&snapshot), blink_on, &mut out);
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
            self.connection = None;
            (self.notify)(ControllerEvent::Status(self.status()));
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
        assert_eq!(lamp_state(&s).transition_fx, Lamp::Off);
        s.transition.deck = Some(DeckId::A);
        assert_eq!(lamp_state(&s).transition_fx, Lamp::On);
        s.transition.releasing = true;
        assert_eq!(lamp_state(&s).transition_fx, Lamp::Blink);
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
            "arrives with the library"
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
}
