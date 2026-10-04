//! Owns the engine handle, the output stream, the loader thread and the 60 Hz bridge.
//!
//! Locks in this file are taken only on control threads (Tauri commands, loader, bridge);
//! the audio thread talks to the engine exclusively through lock-free queues.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use rille_core::{Command, DeckId};
use rille_engine::output::{self, OutputError, OutputRequest, OutputStream, StreamFailure};
use rille_engine::{EngineEvent, EngineHandle, EngineSlot, engine_pair};

use crate::dto::{AudioStatus, Deck, DeckLoadFailed, DeckLoaded, StateFrame};

/// Largest block the engine renders at once; device buffers above this are split.
const MAX_BLOCK: usize = 1024;
/// Bridge period: 60 Hz.
const FRAME_INTERVAL: Duration = Duration::from_micros(16_667);
const PREFERRED_RATES: [u32; 2] = [48_000, 44_100];

/// Notifications for the frontend. Mapped to Tauri events in `lib.rs`.
#[derive(Debug, Clone)]
pub enum UiEvent {
    State(StateFrame),
    DeckLoaded(DeckLoaded),
    DeckLoadFailed(DeckLoadFailed),
    DeckEnded(Deck),
    AudioChanged(AudioStatus),
}

pub type Notify = Arc<dyn Fn(UiEvent) + Send + Sync>;

struct LoadRequest {
    deck: DeckId,
    path: PathBuf,
    id: u64,
}

enum Control {
    StreamFailed(StreamFailure),
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
    loader_tx: Mutex<Sender<LoadRequest>>,
    control_tx: Mutex<Sender<Control>>,
    next_track_id: AtomicU64,
    notify: Notify,
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

/// After a stream failure: fall back to the system default device, keep the buffer size.
pub fn fallback_request(current: &OutputRequest) -> OutputRequest {
    OutputRequest {
        device_id: None,
        ..current.clone()
    }
}

pub fn failure_message(failure: &StreamFailure, device: Option<&str>) -> String {
    let name = device.unwrap_or("Das Audio-Gerät");
    match failure {
        StreamFailure::DeviceGone => {
            format!("{name} wurde getrennt. Wechsel auf das Standardgerät.")
        }
        StreamFailure::Other(e) => {
            format!("Audio-Ausgabe unterbrochen ({e}). Neustart der Ausgabe.")
        }
    }
}

/// Track title for display: the file name without extension.
pub fn title_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unbenannt")
        .to_string()
}

impl AudioService {
    pub fn start(notify: Notify) -> AudioService {
        let sample_rate = pick_engine_rate(&output::device_rates(None));
        let (engine, handle) = engine_pair(sample_rate, MAX_BLOCK);
        let (loader_tx, loader_rx) = mpsc::channel();
        let (control_tx, control_rx) = mpsc::channel();
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
                loader_tx: Mutex::new(loader_tx),
                control_tx: Mutex::new(control_tx),
                next_track_id: AtomicU64::new(1),
                notify,
            }),
        };
        service.spawn_loader(loader_rx);
        service.spawn_bridge(control_rx);
        let request = lock(&service.shared.output).request.clone();
        if let Err(e) = service.open(request) {
            tracing::warn!(%e, "no audio output at start");
        }
        service
    }

    pub fn sample_rate(&self) -> u32 {
        self.shared.sample_rate
    }

    pub fn send(&self, command: Command) {
        lock(&self.shared.handle).send(command);
    }

    /// Queue a file for decoding. Returns the track id the deck will report once loaded.
    pub fn load_file(&self, deck: DeckId, path: PathBuf) -> Result<u64, String> {
        let id = self.shared.next_track_id.fetch_add(1, Ordering::Relaxed);
        lock(&self.shared.loader_tx)
            .send(LoadRequest { deck, path, id })
            .map_err(|_| "Der Ladevorgang ist nicht verfügbar".to_string())?;
        Ok(id)
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
            error: out.error.clone(),
        }
    }

    /// (Re)open the output. The previous stream stops first, so only one stream ever renders.
    pub fn open(&self, request: OutputRequest) -> Result<AudioStatus, OutputError> {
        {
            let mut out = lock(&self.shared.output);
            out.stream = None;
            out.request = request.clone();
            let control_tx = lock(&self.shared.control_tx).clone();
            match output::open(&self.shared.slot, &request, move |failure| {
                let _ = control_tx.send(Control::StreamFailed(failure));
            }) {
                Ok(stream) => {
                    tracing::info!(info = ?stream.info, "audio output open");
                    out.stream = Some(stream);
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

    fn spawn_loader(&self, rx: Receiver<LoadRequest>) {
        let shared = self.shared.clone();
        let spawned = thread::Builder::new()
            .name("rille-loader".into())
            .spawn(move || {
                for request in rx {
                    let title = title_from_path(&request.path);
                    let deck = Deck::from(request.deck);
                    match rille_library::decode_file(&request.path, shared.sample_rate, request.id)
                    {
                        Ok(track) => {
                            let duration_secs = track.duration_secs();
                            let mut track = Arc::new(track);
                            // The track queue holds several loads; retry briefly if it is full.
                            let mut attempts = 0;
                            loop {
                                match lock(&shared.handle).load(request.deck, track) {
                                    Ok(()) => break,
                                    Err(back) if attempts < 100 => {
                                        track = back;
                                        attempts += 1;
                                        thread::sleep(Duration::from_millis(5));
                                    }
                                    Err(_) => {
                                        tracing::error!("track queue stuck, dropping load");
                                        break;
                                    }
                                }
                            }
                            (shared.notify)(UiEvent::DeckLoaded(DeckLoaded {
                                deck,
                                track_id: request.id,
                                title,
                                duration_secs,
                            }));
                        }
                        Err(e) => {
                            tracing::warn!(path = %request.path.display(), %e, "decode failed");
                            (shared.notify)(UiEvent::DeckLoadFailed(DeckLoadFailed {
                                deck,
                                title,
                                message: e.to_string(),
                            }));
                        }
                    }
                }
            });
        if let Err(e) = spawned {
            tracing::error!(%e, "could not start loader thread");
        }
    }

    fn spawn_bridge(&self, control_rx: Receiver<Control>) {
        let service = self.clone();
        let spawned = thread::Builder::new()
            .name("rille-bridge".into())
            .spawn(move || {
                let mut last_sent: Option<rille_core::Snapshot> = None;
                let mut next = Instant::now();
                loop {
                    next += FRAME_INTERVAL;
                    let (snapshot, dropped, events) = {
                        let mut handle = lock(&service.shared.handle);
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
                            (service.shared.notify)(UiEvent::DeckEnded(deck.into()));
                        }
                    }
                    let playing = snapshot.decks.iter().any(|d| d.playing);
                    let peaks_alive = snapshot.master_peak.iter().any(|p| *p > 0.0);
                    if last_sent != Some(snapshot)
                        && (playing || peaks_alive || last_sent.is_none() || {
                            let prev = last_sent.unwrap_or_default();
                            prev.decks != snapshot.decks
                                || prev.channel_fader != snapshot.channel_fader
                                || prev.crossfader != snapshot.crossfader
                                || prev.curve != snapshot.curve
                                || prev.trim != snapshot.trim
                                || prev.master_gain != snapshot.master_gain
                                || prev.xruns != snapshot.xruns
                        })
                    {
                        (service.shared.notify)(UiEvent::State(StateFrame::from_snapshot(
                            &snapshot, dropped,
                        )));
                        last_sent = Some(snapshot);
                    }

                    let timeout = next.saturating_duration_since(Instant::now());
                    match control_rx.recv_timeout(timeout) {
                        Ok(Control::StreamFailed(failure)) => service.recover(failure),
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => {}
                    }
                    if Instant::now() > next + FRAME_INTERVAL * 4 {
                        next = Instant::now(); // fell behind (e.g. machine slept): don't burst
                    }
                }
            });
        if let Err(e) = spawned {
            tracing::error!(%e, "could not start bridge thread");
        }
    }

    fn recover(&self, failure: StreamFailure) {
        let (request, device_name) = {
            let out = lock(&self.shared.output);
            let name = out.stream.as_ref().map(|s| s.info.device_name.clone());
            (fallback_request(&out.request), name)
        };
        let message = failure_message(&failure, device_name.as_deref());
        tracing::warn!(?failure, "audio stream failed, reopening");
        // CoreAudio may report a vanished device a moment before the default switches.
        thread::sleep(Duration::from_millis(250));
        let result = self.open(request);
        let mut out = lock(&self.shared.output);
        out.error = Some(match result {
            Ok(_) => message,
            Err(e) => format!("{message} {e}"),
        });
        drop(out);
        (self.shared.notify)(UiEvent::AudioChanged(self.status()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_keeps_buffer_and_rate_but_uses_default_device() {
        let current = OutputRequest {
            device_id: Some("coreaudio:usb".into()),
            buffer_frames: 128,
            sample_rate: 48_000,
        };
        let next = fallback_request(&current);
        assert_eq!(next.device_id, None);
        assert_eq!((next.buffer_frames, next.sample_rate), (128, 48_000));
    }

    #[test]
    fn failure_messages_are_german_and_name_the_device() {
        let m = failure_message(&StreamFailure::DeviceGone, Some("Interface"));
        assert!(m.contains("Interface") && m.contains("getrennt"));
        assert!(failure_message(&StreamFailure::Other("x".into()), None).contains("unterbrochen"));
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
}
