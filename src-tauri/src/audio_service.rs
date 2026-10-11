//! Owns the engine handle, the output stream, the loader thread and the 60 Hz bridge.
//!
//! Locks in this file are taken only on control threads (Tauri commands, loader, bridge,
//! recovery); the audio thread talks to the engine exclusively through lock-free queues and
//! reports stream failures through atomics.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use rille_core::{BeatGrid, Command, DeckCommand, DeckId, HOT_CUES, TrackAudio};
use rille_engine::output::{self, OutputError, OutputRequest, OutputStream, StreamFailure};
use rille_engine::{EngineEvent, EngineHandle, EngineSlot, TrackLoad, engine_pair};
use rille_library::decode::{DecodeStream, open_stream};

use crate::dto::{AudioStatus, Deck, DeckLoadFailed, DeckLoaded, StateFrame};

/// Largest block the engine renders at once; device buffers above this are split.
const MAX_BLOCK: usize = 1024;
/// Bridge period: 60 Hz.
const FRAME_INTERVAL: Duration = Duration::from_micros(16_667);
const PREFERRED_RATES: [u32; 2] = [48_000, 44_100];
/// Wait before reopening after a failure: CoreAudio reports a vanished device a moment before
/// the system default switches.
const RECOVERY_DELAY: Duration = Duration::from_millis(250);

/// Notifications for the frontend. Mapped to Tauri events in `lib.rs`.
#[derive(Debug, Clone)]
pub enum UiEvent {
    /// Boxed: a frame is large, the other events are small.
    State(Box<StateFrame>),
    DeckLoaded(DeckLoaded),
    DeckLoadFailed(DeckLoadFailed),
    /// The track plays, but not all of it could be decoded.
    DeckLoadWarning(DeckLoadFailed),
    DeckEnded(Deck),
    /// Hot cues of a library track changed on a deck (set or deleted): save them.
    HotCuesChanged {
        library_id: i64,
        cues: [Option<f64>; HOT_CUES],
    },
    AudioChanged(AudioStatus),
}

pub type Notify = Arc<dyn Fn(UiEvent) + Send + Sync>;

struct LoadRequest {
    deck: DeckId,
    path: PathBuf,
    id: u64,
    meta: TrackMeta,
}

/// What the deck shows about a track: from the library, or just the file name.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackMeta {
    pub title: String,
    pub artist: Option<String>,
    pub bpm: Option<f64>,
    /// Library row, for the waveform and the beatgrid.
    pub library_id: Option<i64>,
    pub grid: Option<BeatGrid>,
    /// Saved hot cues, in seconds.
    pub hot_cues: [Option<f64>; HOT_CUES],
}

impl TrackMeta {
    pub fn from_path(path: &Path) -> TrackMeta {
        TrackMeta {
            title: title_from_path(path),
            artist: None,
            bpm: None,
            library_id: None,
            grid: None,
            hot_cues: [None; HOT_CUES],
        }
    }
}

struct OutputState {
    stream: Option<OutputStream>,
    request: OutputRequest,
    error: Option<String>,
}

struct Shared {
    handle: Mutex<EngineHandle>,
    slot: EngineSlot,
    output: Mutex<OutputState>,
    sample_rate: u32,
    /// One loader thread per deck, so loading one deck never waits for the other.
    loader_tx: [Mutex<Sender<LoadRequest>>; 2],
    next_track_id: AtomicU64,
    /// Most recent load request per deck; 0 = none (or cancelled by unload).
    latest_request: [AtomicU64; 2],
    recovering: AtomicBool,
    notify: Notify,
    /// What is on each deck, for grid updates and saving hot cues.
    deck_tracks: [Mutex<Option<DeckTrack>>; 2],
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct DeckTrack {
    /// The load request id (= engine track id).
    track_id: u64,
    library_id: Option<i64>,
}

#[derive(Clone)]
pub struct AudioService {
    shared: Arc<Shared>,
}

/// Recover from poisoned locks: a panic elsewhere must not take the audio controls down.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Sample rate the engine runs at for its whole life: the first preferred rate the given
/// device supports, else 48 kHz (opening will then report a clear error).
pub fn pick_engine_rate(supported: &[u32]) -> u32 {
    PREFERRED_RATES
        .into_iter()
        .find(|r| supported.contains(r))
        .unwrap_or(PREFERRED_RATES[0])
}

/// What to open after a failure: a vanished device falls back to the system default; any other
/// failure retries the same device.
pub fn recovery_request(current: &OutputRequest, failure: StreamFailure) -> OutputRequest {
    match failure {
        StreamFailure::DeviceGone => OutputRequest {
            device_id: None,
            ..current.clone()
        },
        StreamFailure::Other => current.clone(),
    }
}

pub fn failure_message(failure: StreamFailure, device: Option<&str>) -> String {
    let name = device.unwrap_or("Das Audio-Gerät");
    match failure {
        StreamFailure::DeviceGone => {
            format!("{name} wurde getrennt. Wechsel auf das Standardgerät.")
        }
        StreamFailure::Other => format!("Audio-Ausgabe von {name} unterbrochen, neu gestartet."),
    }
}

/// Track title for display: the file name without extension.
pub fn title_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unbenannt")
        .to_string()
}

const DECODER_PANIC: &str = "Der Decoder ist an dieser Datei gescheitert";

/// Open for streaming decode, turning a panic inside the decoder into an error so the loader
/// thread survives.
fn open_guarded(path: &Path, rate: u32, id: u64) -> Result<DecodeStream, String> {
    match catch_unwind(AssertUnwindSafe(|| open_stream(path, rate, id))) {
        Ok(Ok(stream)) => Ok(stream),
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err(DECODER_PANIC.to_string()),
    }
}

/// Fill the track while it plays. Whatever happens, the track ends up complete (with what
/// could be decoded), so the deck never waits for audio that will not come.
fn run_guarded(
    stream: DecodeStream,
    track: &TrackAudio,
    keep_going: impl FnMut() -> bool,
) -> Result<(), String> {
    match catch_unwind(AssertUnwindSafe(|| stream.run(keep_going))) {
        Ok(result) => result.map_err(|e| e.to_string()),
        Err(_) => {
            track.finish(track.ready_frames());
            Err(DECODER_PANIC.to_string())
        }
    }
}

impl AudioService {
    pub fn start(notify: Notify) -> AudioService {
        let sample_rate = pick_engine_rate(&output::device_rates(None));
        let (engine, handle) = engine_pair(sample_rate, MAX_BLOCK);
        let (tx_a, rx_a) = mpsc::channel();
        let (tx_b, rx_b) = mpsc::channel();
        let service = AudioService {
            shared: Arc::new(Shared {
                handle: Mutex::new(handle),
                slot: EngineSlot::new(engine),
                output: Mutex::new(OutputState {
                    stream: None,
                    request: OutputRequest {
                        device_id: None,
                        buffer_frames: output::DEFAULT_BUFFER,
                        sample_rate,
                    },
                    error: None,
                }),
                sample_rate,
                loader_tx: [Mutex::new(tx_a), Mutex::new(tx_b)],
                next_track_id: AtomicU64::new(1),
                latest_request: [AtomicU64::new(0), AtomicU64::new(0)],
                recovering: AtomicBool::new(false),
                notify,
                deck_tracks: [Mutex::new(None), Mutex::new(None)],
            }),
        };
        service.spawn_loader(DeckId::A, rx_a);
        service.spawn_loader(DeckId::B, rx_b);
        service.spawn_bridge();
        let request = lock(&service.shared.output).request.clone();
        if let Err(e) = service.open(request) {
            tracing::warn!(%e, "no audio output at start");
        }
        service
    }

    pub fn sample_rate(&self) -> u32 {
        self.shared.sample_rate
    }

    /// The engine's second command queue, for the controller thread (once).
    pub fn take_controller_sender(&self) -> Option<rille_engine::CommandSender> {
        lock(&self.shared.handle).take_midi_sender()
    }

    /// Latest engine state (for controller LEDs and soft takeover).
    pub fn snapshot(&self) -> rille_core::Snapshot {
        lock(&self.shared.handle).snapshot()
    }

    pub fn send(&self, command: Command) {
        lock(&self.shared.handle).send(command);
    }

    /// Queue a file for decoding. Returns the track id the deck will report once loaded.
    pub fn load_file(&self, deck: DeckId, path: PathBuf, meta: TrackMeta) -> Result<u64, String> {
        let id = self.shared.next_track_id.fetch_add(1, Ordering::Relaxed);
        self.shared.latest_request[deck.index()].store(id, Ordering::Release);
        lock(&self.shared.loader_tx[deck.index()])
            .send(LoadRequest {
                deck,
                path,
                id,
                meta,
            })
            .map_err(|_| "Der Ladevorgang ist nicht verfügbar".to_string())?;
        Ok(id)
    }

    /// A library track's grid changed (analysis finished, user correction): decks playing it
    /// get it at once.
    pub fn update_grid(&self, library_id: i64, grid: Option<BeatGrid>) {
        for deck in DeckId::ALL {
            let on_deck = lock(&self.shared.deck_tracks[deck.index()])
                .is_some_and(|t| t.library_id == Some(library_id));
            if on_deck {
                self.send(Command::Deck(deck, DeckCommand::SetGrid(grid)));
            }
        }
    }

    /// Empty the deck and cancel a load that is still decoding.
    pub fn unload(&self, deck: DeckId) {
        self.shared.latest_request[deck.index()].store(0, Ordering::Release);
        *lock(&self.shared.deck_tracks[deck.index()]) = None;
        self.send(Command::Deck(deck, DeckCommand::Unload));
    }

    pub fn status(&self) -> AudioStatus {
        let out = lock(&self.shared.output);
        let Some(stream) = &out.stream else {
            return AudioStatus {
                connected: false,
                requested_buffer: out.request.buffer_frames,
                sample_rate: self.shared.sample_rate,
                error: out.error.clone(),
                ..AudioStatus::default()
            };
        };
        let info = &stream.info;
        let device_us = stream.stats.device_latency_us.load(Ordering::Relaxed);
        let buffer = info
            .buffer_frames
            .unwrap_or_else(|| stream.stats.last_callback_frames.load(Ordering::Relaxed));
        let buffer_ms = buffer as f32 * 1000.0 / info.sample_rate.max(1) as f32;
        AudioStatus {
            connected: true,
            device_id: Some(info.device_id.clone()),
            device_name: Some(info.device_name.clone()),
            sample_rate: info.sample_rate,
            channels: info.channels,
            requested_buffer: out.request.buffer_frames,
            buffer_frames: info.buffer_frames,
            latency_ms: buffer_ms + device_us as f32 / 1000.0,
            device_xruns: stream.stats.device_xruns.load(Ordering::Relaxed),
            error: out.error.clone(),
        }
    }

    /// Open a new output. The new stream is built first; only when that worked does it replace
    /// the old one, so a failed device switch keeps the music playing.
    pub fn open(&self, request: OutputRequest) -> Result<AudioStatus, OutputError> {
        let result = output::open(&self.shared.slot, &request);
        {
            let mut out = lock(&self.shared.output);
            match result {
                Ok(stream) => {
                    tracing::info!(info = ?stream.info, "audio output open");
                    // Dropping the old stream stops it; the slot keeps the engine meanwhile.
                    out.stream = Some(stream);
                    out.request = request;
                    out.error = None;
                }
                Err(e) => {
                    out.error = Some(e.to_string());
                    drop(out);
                    (self.shared.notify)(UiEvent::AudioChanged(self.status()));
                    return Err(e);
                }
            }
        }
        let status = self.status();
        (self.shared.notify)(UiEvent::AudioChanged(status.clone()));
        Ok(status)
    }

    fn spawn_loader(&self, deck_id: DeckId, rx: Receiver<LoadRequest>) {
        let shared = self.shared.clone();
        let name = format!("rille-loader-{}", deck_id.index() + 1);
        let spawned = thread::Builder::new().name(name).spawn(move || {
            for request in rx {
                let deck = Deck::from(request.deck);
                let title = request.meta.title.clone();
                let is_current = || {
                    shared.latest_request[request.deck.index()].load(Ordering::Acquire)
                        == request.id
                };
                if !is_current() {
                    continue; // superseded or unloaded before decoding started
                }
                let fail = |message: String| {
                    (shared.notify)(UiEvent::DeckLoadFailed(DeckLoadFailed {
                        deck,
                        track_id: request.id,
                        title: title.clone(),
                        message,
                    }))
                };
                // Opening only reads the header; the track is playable right away and fills
                // while it plays.
                let stream = match open_guarded(&request.path, shared.sample_rate, request.id) {
                    Ok(stream) => stream,
                    Err(message) => {
                        tracing::warn!(path = %request.path.display(), %message, "decode failed");
                        fail(message);
                        continue;
                    }
                };
                if !is_current() {
                    continue;
                }
                let track = stream.track();
                let duration_secs = track.duration_secs();
                let rate = f64::from(shared.sample_rate);
                let mut pending = Some(TrackLoad {
                    grid: request.meta.grid,
                    hot_cues: request
                        .meta
                        .hot_cues
                        .map(|c| c.filter(|s| s.is_finite() && *s >= 0.0).map(|s| (s * rate) as u64)),
                    ..TrackLoad::new(request.deck, track.clone())
                });
                for _ in 0..100 {
                    let Some(t) = pending.take() else { break };
                    // Lock only for the push itself; never sleep while holding it.
                    let pushed = lock(&shared.handle).load_track(t);
                    if let Err(back) = pushed {
                        pending = Some(back);
                        thread::sleep(Duration::from_millis(5));
                    }
                }
                if pending.is_some() {
                    fail("Die Audio-Engine nimmt gerade keine Tracks an (keine Ausgabe aktiv?)".into());
                    continue;
                }
                *lock(&shared.deck_tracks[request.deck.index()]) = Some(DeckTrack {
                    track_id: request.id,
                    library_id: request.meta.library_id,
                });
                (shared.notify)(UiEvent::DeckLoaded(DeckLoaded {
                    deck,
                    track_id: request.id,
                    title,
                    artist: request.meta.artist.clone(),
                    bpm: request.meta.bpm,
                    library_id: request.meta.library_id,
                    duration_secs,
                }));
                // A newer load or an unload on this deck stops the decode; what is decoded stays.
                if let Err(message) = run_guarded(stream, &track, is_current) {
                    tracing::warn!(path = %request.path.display(), %message, "decode stopped early");
                    // Cancelled loads (newer load, eject) are not news.
                    if is_current() {
                        (shared.notify)(UiEvent::DeckLoadWarning(DeckLoadFailed {
                            deck,
                            track_id: request.id,
                            title: request.meta.title.clone(),
                            message,
                        }));
                    }
                }
            }
        });
        if let Err(e) = spawned {
            tracing::error!(%e, "could not start loader thread");
        }
    }

    fn spawn_bridge(&self) {
        let service = self.clone();
        let spawned = thread::Builder::new()
            .name("rille-bridge".into())
            .spawn(move || {
                let mut last_sent: Option<rille_core::Snapshot> = None;
                let mut cues = [SeenCues::default(); 2];
                let mut next = Instant::now();
                loop {
                    next += FRAME_INTERVAL;
                    service.bridge_tick(&mut last_sent, &mut cues);
                    service.check_stream();
                    let now = Instant::now();
                    if now > next + FRAME_INTERVAL * 4 {
                        next = now; // fell behind (e.g. machine slept): don't burst
                    }
                    thread::sleep(next.saturating_duration_since(Instant::now()));
                }
            });
        if let Err(e) = spawned {
            tracing::error!(%e, "could not start bridge thread");
        }
    }

    fn bridge_tick(&self, last_sent: &mut Option<rille_core::Snapshot>, cues: &mut [SeenCues; 2]) {
        let (snapshot, dropped, events) = {
            let mut handle = lock(&self.shared.handle);
            let snapshot = handle.snapshot();
            handle.collect_garbage();
            let mut events = [None; 8];
            for slot in &mut events {
                *slot = handle.next_event();
                if slot.is_none() {
                    break;
                }
            }
            (snapshot, handle.dropped_commands(), events)
        };
        for event in events.into_iter().flatten() {
            if let EngineEvent::TrackEnded { deck } = event {
                (self.shared.notify)(UiEvent::DeckEnded(deck.into()));
            }
        }
        for deck in DeckId::ALL {
            let on_deck = *lock(&self.shared.deck_tracks[deck.index()]);
            let d = &snapshot.decks[deck.index()];
            if let Some((library_id, frames)) = cue_change(&mut cues[deck.index()], d, on_deck) {
                let rate = f64::from(snapshot.sample_rate.max(1));
                (self.shared.notify)(UiEvent::HotCuesChanged {
                    library_id,
                    cues: frames.map(|c| c.map(|f| f as f64 / rate)),
                });
            }
        }
        if should_send(last_sent.as_ref(), &snapshot) {
            (self.shared.notify)(UiEvent::State(Box::new(StateFrame::from_snapshot(
                &snapshot, dropped,
            ))));
            *last_sent = Some(snapshot);
        }
    }

    /// Look at the *current* stream only: failures of streams already replaced cannot trigger a
    /// recovery that would tear down a healthy one.
    fn check_stream(&self) {
        let failure = {
            let out = lock(&self.shared.output);
            out.stream.as_ref().and_then(|s| s.stats.failure())
        };
        let Some(failure) = failure else { return };
        if self.shared.recovering.swap(true, Ordering::AcqRel) {
            return;
        }
        let service = self.clone();
        let spawned = thread::Builder::new()
            .name("rille-recover".into())
            .spawn(move || {
                service.recover(failure);
                service.shared.recovering.store(false, Ordering::Release);
            });
        if let Err(e) = spawned {
            tracing::error!(%e, "could not start recovery");
            self.shared.recovering.store(false, Ordering::Release);
        }
    }

    fn recover(&self, failure: StreamFailure) {
        let (request, device_name) = {
            let mut out = lock(&self.shared.output);
            let name = out.stream.as_ref().map(|s| s.info.device_name.clone());
            // The broken stream is useless; drop it now so it stops reporting.
            out.stream = None;
            (recovery_request(&out.request, failure), name)
        };
        let message = failure_message(failure, device_name.as_deref());
        tracing::warn!(?failure, "audio stream failed, reopening");
        thread::sleep(RECOVERY_DELAY);
        let result = self.open(request);
        lock(&self.shared.output).error = Some(match result {
            Ok(_) => message,
            Err(e) => format!("{message} {e}"),
        });
        (self.shared.notify)(UiEvent::AudioChanged(self.status()));
    }
}

/// Hot cues last seen on a deck, per engine track.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SeenCues {
    track_id: Option<u64>,
    cues: [Option<u64>; HOT_CUES],
}

/// Hot cues the DJ changed on the track that is on the deck, to be saved for its library row.
/// The first sight of a track only records its cues (they came from the library); snapshots
/// that still show the previous track are ignored.
fn cue_change(
    seen: &mut SeenCues,
    deck: &rille_core::DeckSnapshot,
    on_deck: Option<DeckTrack>,
) -> Option<(i64, [Option<u64>; HOT_CUES])> {
    let on_deck = on_deck?;
    if deck.track_id != Some(on_deck.track_id) {
        return None;
    }
    let first_sight = seen.track_id != deck.track_id;
    let changed = seen.cues != deck.hot_cues;
    *seen = SeenCues {
        track_id: deck.track_id,
        cues: deck.hot_cues,
    };
    if first_sight || !changed {
        return None;
    }
    on_deck.library_id.map(|id| (id, deck.hot_cues))
}

/// Send a frame when anything visible changed or while audio moves.
fn should_send(last: Option<&rille_core::Snapshot>, now: &rille_core::Snapshot) -> bool {
    let Some(prev) = last else { return true };
    if prev == now {
        return false;
    }
    let moving = now.decks.iter().any(|d| d.playing)
        || now.master_peak.iter().any(|p| *p > 0.0)
        || now.decks.iter().any(|d| d.peak.iter().any(|p| *p > 0.0));
    moving
        || prev.decks != now.decks
        || prev.channel_fader != now.channel_fader
        || prev.trim != now.trim
        || prev.crossfader != now.crossfader
        || prev.curve != now.curve
        || prev.master_gain != now.master_gain
        || prev.xruns != now.xruns
        || prev.sample_rate != now.sample_rate
}

#[cfg(test)]
mod tests {
    use super::*;
    use rille_core::Snapshot;

    fn request(device: Option<&str>) -> OutputRequest {
        OutputRequest {
            device_id: device.map(str::to_string),
            buffer_frames: 128,
            sample_rate: 48_000,
        }
    }

    #[test]
    fn vanished_device_falls_back_to_default() {
        let next = recovery_request(&request(Some("coreaudio:usb")), StreamFailure::DeviceGone);
        assert_eq!(next, request(None));
    }

    #[test]
    fn other_failures_retry_the_same_device() {
        let current = request(Some("coreaudio:usb"));
        assert_eq!(recovery_request(&current, StreamFailure::Other), current);
    }

    #[test]
    fn failure_messages_are_german_and_name_the_device() {
        let m = failure_message(StreamFailure::DeviceGone, Some("Interface"));
        assert!(m.contains("Interface") && m.contains("getrennt"));
        assert!(failure_message(StreamFailure::Other, None).contains("unterbrochen"));
    }

    #[test]
    fn engine_rate_prefers_48k_then_44k1() {
        assert_eq!(pick_engine_rate(&[44_100, 48_000, 96_000]), 48_000);
        assert_eq!(pick_engine_rate(&[44_100]), 44_100);
        assert_eq!(pick_engine_rate(&[]), 48_000);
    }

    #[test]
    fn title_is_file_stem() {
        assert_eq!(
            title_from_path(Path::new("/m/Artist - Song.mp3")),
            "Artist - Song"
        );
        assert_eq!(title_from_path(Path::new("/")), "Unbenannt");
    }

    #[test]
    fn frames_are_sent_on_change_and_while_moving_only() {
        let idle = Snapshot::default();
        assert!(should_send(None, &idle), "first frame always");
        assert!(!should_send(Some(&idle), &idle), "nothing changed");
        let mut faded = idle;
        faded.crossfader = 0.3;
        assert!(should_send(Some(&idle), &faded));
        let mut clock_only = idle;
        clock_only.frame_clock = 999;
        assert!(
            !should_send(Some(&idle), &clock_only),
            "the clock alone is not news"
        );
        let mut playing = clock_only;
        playing.decks[0].playing = true;
        let mut later = playing;
        later.frame_clock += 256;
        assert!(
            should_send(Some(&playing), &later),
            "playing decks stream continuously"
        );
    }

    #[test]
    fn hot_cues_are_saved_only_when_the_dj_changes_them() {
        let mut seen = SeenCues::default();
        let on_deck = Some(DeckTrack {
            track_id: 7,
            library_id: Some(42),
        });
        let mut deck = Snapshot::default().decks[0];
        deck.track_id = Some(6); // the previous track is still showing
        deck.hot_cues[0] = Some(5);
        assert_eq!(cue_change(&mut seen, &deck, on_deck), None);
        deck.track_id = Some(7);
        deck.hot_cues = [None; HOT_CUES];
        deck.hot_cues[1] = Some(100);
        assert_eq!(
            cue_change(&mut seen, &deck, on_deck),
            None,
            "loaded cues are not news"
        );
        assert_eq!(
            cue_change(&mut seen, &deck, on_deck),
            None,
            "nothing changed"
        );
        deck.hot_cues[2] = Some(200);
        let (id, cues) = cue_change(&mut seen, &deck, on_deck).unwrap();
        assert_eq!((id, cues[2]), (42, Some(200)));
        deck.hot_cues[1] = None;
        assert!(
            cue_change(&mut seen, &deck, on_deck).is_some(),
            "deleting is a change too"
        );
        // A file played without the library has nowhere to save to.
        deck.hot_cues[3] = Some(9);
        let no_library = Some(DeckTrack {
            track_id: 7,
            library_id: None,
        });
        assert_eq!(cue_change(&mut seen, &deck, no_library), None);
    }
}
