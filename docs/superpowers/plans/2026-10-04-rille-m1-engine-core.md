# M1 – Engine-Kern: Wiedergabe und Mixer-Grundlage

> **Für agentische Ausführung:** Teilplan zu `2026-10-04-rille-masterplan.md`. Ausführung inline
> (superpowers:executing-plans), TDD je Task, Review am Meilenstein-Ende.

**Ziel:** Eine Datei aus dem Finder auf ein Deck ziehen, abspielen, mit Cue (Pioneer-Verhalten),
Kanalfader und Crossfader mischen, Pegel sehen; Gerät und Puffer wählen; Abziehen des Geräts
übersteht die App.

**Architektur:** `rille-core` definiert `Command`, `Snapshot`, `TrackAudio`. `rille-engine` enthält
die reine DSP-/Zustandslogik (`Engine::process`, offline testbar) plus `output` (cpal) und
`EngineSlot` (wait-freie Übergabe der Engine zwischen Streams). `rille-library::decode` dekodiert
(symphonia) und resampelt (rubato). `src-tauri` hält `AudioService` (Loader-Thread, Bridge-Thread,
Commands) und streamt `StateFrame`s per Channel.

**Tech-Stack:** cpal 0.18, symphonia 0.6 (Features mp3, aac, alac, flac, pcm, wav, aiff, isomp4),
rubato 5, rtrb 0.4, triple_buffer 9, assert_no_alloc 1.1, thiserror.

## Review Focus (aus dem Masterplan, für M1 zuständig)

1. Gerät verschwindet → Stream-Fehler-Pfad baut neu auf, Engine-Zustand (geladene Tracks,
   Positionen, Cues) bleibt erhalten. Test: `EngineSlot` übergibt dieselbe Engine an einen
   zweiten Konsumenten; `AudioService::handle_stream_error` wählt Default-Gerät und meldet es.
2. Nicht dekodierbare Datei (0 Byte, Text mit .mp3-Endung, abgeschnittene WAV) → `DecodeError`,
   Deck unverändert, UI-Meldung. Tests in `decode`.

## Dateien

```
crates/rille-core/src/{lib.rs, command.rs, snapshot.rs, track.rs, mixer.rs}
crates/rille-engine/src/{lib.rs, engine.rs, deck.rs, cue.rs, mixer.rs, smooth.rs, meter.rs,
                         slot.rs, output.rs, queues.rs}
crates/rille-engine/tests/no_alloc.rs
crates/rille-library/src/{lib.rs, decode.rs, resample.rs}  + tests/fixtures-Generator in Tests
src-tauri/src/{lib.rs, audio_service.rs, bridge.rs, commands.rs, state_frame.rs}
src/ipc/{types.ts, tauri.ts, mock.ts, index.ts}
src/state/{store.ts, store.test.ts, extrapolate.ts, extrapolate.test.ts}
src/deck/{DeckPanel.tsx, Transport.tsx, TimeReadout.tsx}  src/mixer/{MixerPanel.tsx, Fader.tsx, Meter.tsx}
src/settings/{SettingsDialog.tsx, AudioSettings.tsx}
```

## Schnittstellen (verbindlich für alle Tasks)

```rust
// rille-core
pub struct TrackAudio { pub id: u64, pub sample_rate: u32, pub frames: usize, pub samples: Vec<f32> } // stereo interleaved
pub enum CrossfaderCurve { Smooth, Linear, Cut }
pub enum DeckCommand { PlayPause, Play, Pause, CuePress, CueRelease, JumpToStart, Seek { frame: u64 } }
pub enum MixerCommand { ChannelFader(DeckId, f32), Crossfader(f32 /* 0=links,1=rechts */),
                        CrossfaderCurve(CrossfaderCurve), MasterGain(f32), Trim(DeckId, f32) }
pub enum Command { Deck(DeckId, DeckCommand), Mixer(MixerCommand), Unload(DeckId) }
pub struct DeckSnapshot { pub track_id: Option<u64>, pub position: u64, pub frames: u64,
                          pub playing: bool, pub cue: u64, pub previewing: bool,
                          pub peak: [f32; 2] }
pub struct Snapshot { pub frame_clock: u64, pub sample_rate: u32, pub decks: [DeckSnapshot; 2],
                      pub channel_fader: [f32; 2], pub crossfader: f32, pub curve: CrossfaderCurve,
                      pub master_gain: f32, pub master_peak: [f32; 2],
                      pub xruns: u32, pub dropped_commands: u32 }

// rille-engine
impl Engine { pub fn new(sample_rate: u32, max_block: usize, io: EngineIo) -> Engine;
              pub fn process(&mut self, out: &mut [f32], channels: usize); }
pub struct EngineHandle { /* ui commands producer, track producer, garbage consumer,
                             snapshot output, events consumer */ }
pub fn engine_pair(sample_rate: u32, max_block: usize) -> (Engine, EngineHandle);
pub struct EngineSlot; // Arc<…>, try_with(|&mut Engine|) wait-free, nie zwei Nutzer gleichzeitig
pub mod output { pub fn list_devices() -> Vec<DeviceInfo>;
                 pub struct OutputStream; pub fn open(slot, device_id: Option<&str>, buffer_frames: u32,
                                                     on_error: impl Fn(StreamFailure)) -> Result<OutputStream, OutputError>; }

// rille-library
pub fn decode_file(path: &Path, target_rate: u32, id: u64) -> Result<TrackAudio, DecodeError>;
```

## Tasks

### Task 1 – Core-Typen
Typen oben in `rille-core`, alle `Copy` außer `TrackAudio`. Test: `size_of::<Command>() <= 16`,
`Snapshot: Copy + Default`.

### Task 2 – Glätter und Fader-/Crossfader-Kennlinien (`smooth.rs`, `mixer.rs`)
- `Smoother::new(sample_rate, ms)`, `set_target`, `next() -> f32`, `snap`.
  Tests: Sprung 0→1 erzeugt zwischen zwei Samples keinen Schritt > 0,01 bei 10 ms/48 kHz;
  nach 5τ innerhalb 1 % des Ziels; `snap` setzt sofort.
- `channel_gain(v) = v²` (v∈[0,1]); `crossfader_gains(x, curve) -> (l, r)`.
  Tests: Endpunkte (x=0 → (1,0), x=1 → (0,1)) für alle Kurven; Smooth: l²+r² = 1 überall
  (konstante Leistung), Mitte je 0,707; Linear: l+r = 1; Cut: l = 1 für x ≤ 0,95, r = 1 für
  x ≥ 0,05 (Scratch-Kurve); Werte außerhalb [0,1] werden geklemmt.

### Task 3 – Cue-Logik (`cue.rs`) als reine Zustandsmaschine
Eingaben: `press`, `release`, `play`, Zustand `{playing, position, cue, previewing}`.
Pioneer-Verhalten, je ein Test:
1. Pausiert, nicht am Cue, `press` → Cue = Position (und bleibt pausiert, Vorhören startet).
2. Pausiert am Cue, `press` halten → spielt (previewing), `release` → zurück zum Cue, pausiert.
3. Während previewing `play` → beim `release` läuft es weiter (previewing endet).
4. Spielend, `press` → springt zum Cue und pausiert.
5. `JumpToStart` → Position 0, Abspielzustand bleibt.
6. Cue jenseits des Trackendes ist unmöglich (geklemmt).

### Task 4 – Deck und Engine (`deck.rs`, `engine.rs`, `meter.rs`, `queues.rs`)
- Deck liest stereo frames aus `Arc<TrackAudio>` (Rate 1,0 in M1; Position als u64), Ende → stoppt
  und meldet `Event::TrackEnded`.
- Engine: leert UI- und MIDI-Queue, nimmt Track-Ladungen an (altes `Arc` → Garbage-Queue; ist
  sie voll, bleibt es im Slot `pending_drop` bis zum nächsten Block), mischt Decks über
  Trim·Kanalfader·Crossfader·Master (je geglättet), schreibt Kanäle 0/1, nullt weitere Kanäle,
  Peak-Meter mit Abfall (≈ 20 dB/s), schreibt Snapshot in den Triple Buffer.
- Tests (offline): DC-Track 0,5 → Ausgabe 0,5·Gains; Kanalfader 0 → Stille nach Glättung;
  Track-Ende; Laden während der Wiedergabe gibt das alte Arc über die Garbage-Queue zurück;
  volle Command-Queue zählt `dropped_commands` (Produzentenseite); Ausgabe mit 4 Kanälen lässt 3/4 still.

### Task 5 – Allokationsfreiheit (`tests/no_alloc.rs`)
Integrationstest mit `#[global_allocator] AllocDisabler`: Engine mit geladenen Tracks und
Befehlen in allen Queues; 2000 Blöcke à 256 Frames in `assert_no_alloc(|| engine.process(..))`.
Gegenprobe: ein Test, der absichtlich im Block alloziert, muss abbrechen (`#[should_panic]` geht
bei abort nicht → Gegenprobe als `#[ignore]`-Test dokumentiert und einmal manuell gesehen).

### Task 6 – EngineSlot und cpal-Ausgabe (`slot.rs`, `output.rs`)
- `EngineSlot`: `Arc<{UnsafeCell<Engine>, AtomicBool}>`; `try_with` per `compare_exchange`
  (wait-free), sonst `None` → Callback gibt Stille aus. Test: zwei Threads, nie gleichzeitig drin
  (Zähler-Invariante über 1 Mio Iterationen); nach Drop eines Konsumenten nutzt ein neuer dieselbe
  Engine (Zustand bleibt).
- `output::open`: f32, `BufferSize::Fixed(n)` (Fallback Default, wenn das Gerät ablehnt),
  Error-Callback → `StreamFailure { device_gone: bool }`. `list_devices()` mit Id, Name, Kanälen,
  Default-Flag. Callback: im Debug-Build `assert_no_alloc`, Xrun-Erkennung über Zeitstempel-Lücken
  im `OutputCallbackInfo` (zählt in Snapshot `xruns`). Latenz = Puffer/Rate + gemeldete
  Ausgabelatenz (Zeitstempel `playback - callback`).
- Manuell: echter Ton über das Default-Gerät.

### Task 7 – Dekodieren (`rille-library/decode.rs`, `resample.rs`)
symphonia → f32, Mono auf Stereo dupliziert, >2 Kanäle → erste zwei; rubato (sinc, FFT-basiert)
auf Zielrate, wenn abweichend. Tests mit im Test erzeugten WAVs (eigener Mini-Writer):
44,1 kHz-Sinus 1 kHz → 48 kHz: Länge ±1 Frame, Frequenz per Nulldurchgängen ±0,5 %;
Mono → Stereo; 0-Byte-Datei, Textdatei, abgeschnittene WAV → `DecodeError` mit verständlicher
Meldung (kein Panic). Laufzeit 3-min-WAV dekodieren+resampeln < 1 s (Release, gemessen, nicht
als Test-Schranke).

### Task 8 – AudioService und Bridge (`src-tauri`)
- Hält `EngineHandle`, aktives `OutputStream`, Loader-Thread (mpsc: `LoadRequest{deck, path}`),
  Bridge-Thread (60 Hz: Snapshot lesen, bei Änderung oder Wiedergabe `StateFrame` senden, Garbage
  leeren, Events weiterreichen, Stream-Fehler → Neuaufbau auf Default + Tauri-Event
  `device-changed`).
- Commands: `audio_devices`, `audio_open(device_id, buffer_frames)`, `audio_status`,
  `deck_load_file(deck, path)`, `deck_command(deck, cmd)`, `mixer_command(cmd)`,
  `state_subscribe(channel)`.
- Test (Rust, ohne Gerät): `handle_stream_error` setzt Ziel auf Default und erzeugt Event;
  `StateFrame` serialisiert erwartungsgemäß (camelCase, Felder vollständig).

### Task 9 – Frontend: IPC, Store, Extrapolation
- `src/ipc`: Schnittstelle `Backend` mit `tauri`- und `mock`-Implementierung (Mock spielt einen
  synthetischen Track ab, damit `pnpm dev:web` funktioniert).
- `store.ts`: `useSyncExternalStore`, hält letzten `StateFrame` + Empfangszeit.
- `extrapolate.ts`: `positionAt(frame, now)` = Position + (now − t)·Rate·sampleRate, wenn spielend,
  geklemmt auf Länge. Vitest: pausiert = konstant, spielend wächst linear, Klemme am Ende.

### Task 10 – Frontend: Deck-/Mixer-Oberfläche minimal + Audio-Einstellungen
Deck: Titel (Dateiname), Zeit/Restzeit (tabellarische Ziffern), Play (Kreis↔abgerundetes Quadrat
per Spring), Cue (Pointer-Down/Up = press/release), Drop-Zone für Finder-Dateien
(`onDragDropEvent`, Trefferprüfung über Deck-Rechtecke). Mixer: zwei Kanalfader, Crossfader mit
Kurvenwahl, Pegel (Canvas, nur bei Änderung gezeichnet). Einstellungen-Dialog: Gerät, Puffer
(64…2048), angezeigte Latenz in ms, Fehlermeldung bei Geräteverlust. Leerzustand „kein
Audio-Gerät".
Playwright gegen `pnpm dev:web`: Play/Cue/Fader bedienbar, kein Konsolenfehler.

### Task 11 – Abschluss
`pnpm check`, App manuell: Datei ziehen, spielen, Cue-Verhalten, Fader, Gerätewechsel, USB-Gerät
abziehen (falls vorhanden). README-Status, CLAUDE.md-Fallstricke, Commit, Push, CI grün, Review.
