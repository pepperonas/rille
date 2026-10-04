//! Runs the DDJ-200: hotplug, input → engine commands, engine state → LEDs, MIDI monitor.
//!
//! One thread owns everything controller related. CoreMIDI delivers bytes on its own thread;
//! they are forwarded here through a channel, so the session needs no lock.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use rille_core::{Command, DeckCommand, DeckId, MixerCommand, Snapshot};
use rille_engine::CommandSender;
use rille_midi::ControllerSession;
use rille_midi::ddj200::{Control, ControllerAction, DeckLamps, Lamp, LampState, Scope};
use rille_midi::monitor::{Direction, Monitor, MonitorEntry, describe};
use rille_midi::port::{Connection, DDJ200_PORT, find_input};
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

pub fn lamp_state(snapshot: &Snapshot) -> LampState {
    LampState {
        decks: [
            deck_lamps(snapshot, DeckId::A),
            deck_lamps(snapshot, DeckId::B),
        ],
        ..LampState::default()
    }
}

/// Engine commands for a controller action. Actions whose features arrive in later milestones
/// (tempo, EQ, sync, pads, …) produce nothing yet.
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
        A::ChannelFader { deck: d, value } => {
            vec![Command::Mixer(MixerCommand::ChannelFader(d, value))]
        }
        A::Crossfader(v) => vec![Command::Mixer(MixerCommand::Crossfader(v))],
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
                    Request::Monitor(on) => self.monitor_on = on,
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
                let mut out = Vec::new();
                self.session.on_connect(!audible, &mut out);
                self.sync_software(&snapshot);
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

    fn sync_software(&mut self, s: &Snapshot) {
        self.session
            .sync_software((Scope::Global, Control::Crossfader), s.crossfader);
        for deck in DeckId::ALL {
            let key = (Scope::Deck(deck), Control::ChannelFader);
            self.session
                .sync_software(key, s.channel_fader[deck.index()]);
        }
    }

    fn tick(&mut self) {
        if self.connection.is_some() {
            let snapshot = self.audio.snapshot();
            self.sync_software(&snapshot);
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
    }

    #[test]
    fn monitor_lines_are_hex() {
        let line = MonitorLine::from(MonitorEntry {
            time_us: 5,
            direction: Direction::Out,
            bytes: [0x9F, 0x01, 0x7F],
            len: 3,
            meaning: None,
        });
        assert_eq!((line.hex.as_str(), line.outgoing), ("9F 01 7F", true));
    }
}
