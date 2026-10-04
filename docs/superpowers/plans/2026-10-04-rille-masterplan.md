# rille – Masterplan (Phase 2)

> **Für agentische Ausführung:** Dieser Masterplan legt Architektur, Meilensteine und Dateistruktur fest.
> Vor jedem Meilenstein entsteht ein eigener, kleinteiliger Umsetzungsplan
> (`docs/superpowers/plans/2026-10-04-rille-mN-<name>.md`) mit TDD-Schritten. Ausführung über
> superpowers:executing-plans bzw. superpowers:subagent-driven-development.

**Ziel:** Native macOS-DJ-App (Tauri 2 + Rust + React 19) mit Pioneer DDJ-200 als Primär-Controller,
vollständig auch per Maus und Tastatur bedienbar, gestaltet nach Material 3 Expressive.

**Architektur:** Eine allokationsfreie Rust-Engine rendert im CoreAudio-Callback. Sie bekommt Befehle
über lock-freie SPSC-Queues (`rtrb`) und meldet ihren Zustand als Snapshot zurück. Eine Bridge sendet
diesen Snapshot mit 60 Hz über einen Tauri Channel ans Frontend und an die Controller-LEDs.
Dekodieren, Analyse und Datenbank laufen in Worker-Threads. Das Frontend zeigt nur an und sendet Befehle.

**Tech-Stack:** Rust 1.98 · Tauri 2.12 · cpal 0.18 · midir 0.11 · symphonia 0.6 · rubato 5 · rtrb 0.4 ·
signalsmith-stretch 0.1 (MIT) · rusqlite 0.40 (bundled) · assert_no_alloc 1.1 · tracing ·
React 19 · TypeScript strict · Vite · motion · Vitest · Playwright (gegen Mock-Backend)

**Spec:** Der Projekt-Prompt (Phase 0 bis 3) samt Antworten aus Phase 1 – siehe unten
„Festgelegte Entscheidungen".

## Festgelegte Entscheidungen (Phase 1)

- Lizenz **MIT**, **öffentliches** Repo `github.com/pepperonas/rille`.
- Time-Stretch/Keylock: **Signalsmith Stretch** (MIT), keine GPL-Abhängigkeit.
- Pad-LEDs (nur an oder aus): **leer = aus, belegt = an, aktiv = blinkt im Beat-Takt**.
- DDJ-200 ist vorhanden, aber gerade nicht angeschlossen → Entwicklung gegen einen **virtuellen
  DDJ-200** (CoreMIDI-Virtual-Port); Hardware-Abnahme mit dem Nutzer am Ende von M2 und M7.
- Formate: MP3, AAC/M4A (ohne HE-AAC und DRM), ALAC, FLAC, WAV, AIFF.
- tauri-cli als **lokale devDependency** (`@tauri-apps/cli`), keine globalen Installationen.
- Builds vorerst unsigniert (lokal). Bundle-ID `io.celox.rille`.

## Global Constraints

- macOS 13+, Apple Silicon Hauptziel (`aarch64-apple-darwin`); x86_64 muss bauen, wird aber nicht separat optimiert.
- Audio-Callback: keine Allokation, kein Lock, kein Logging, kein I/O, keine Syscalls. Debug-Builds prüfen das mit `assert_no_alloc`.
- Ziel-Buffer 256 Frames bei 48 kHz, konfigurierbar (64 bis 2048).
- Kein `unwrap()`/`expect()` außerhalb von Tests; `cargo clippy --workspace --all-targets -- -D warnings`; `rustfmt`.
- TypeScript `strict`, ESLint ohne Warnungen.
- Kein Code aus Mixxx (GPL). Mixxx-Mapping nur als Verständnisreferenz.
- Controller-Mapping ausschließlich aus `docs/ddj-200_midi_message_list.pdf`.
- Keine Remote-Inhalte, strikte CSP, minimale Tauri-Capabilities; Fonts lokal (Roboto Flex, OFL).
- Design-Tokens zentral als Single Source of Truth; keine hartkodierten Farben/Abstände/Dauern in Komponenten.
- `prefers-reduced-motion` respektieren; ausschließlich Spring-Easings bzw. M3-Emphasized-Kurven.
- Footer: `© {aktuelles Jahr} Martin Pfeffer | celox.io`, Jahr zur Laufzeit.
- Commits nach Conventional Commits; nach jedem Meilenstein lauffähig, Tests grün.

## Review Focus

Fünf Fehlerfälle, die der Prompt impliziert und die am ehesten einen Nutzer treffen. Jeder bekommt
seinen Test im zuständigen Meilenstein.

1. **Audio-Gerät verschwindet mitten im Set** (USB-Interface abgezogen): kein Absturz, UI meldet es, Wiedereinstieg auf Default-Gerät oder Rückkehr des Geräts. → M1 (Test: Stream-Fehler simulieren, Engine-Zustand bleibt erhalten).
2. **Controller wird abgezogen, während ein Regler bewegt wird** oder nach dem Wiederanstecken steht der Hardware-Fader woanders: kein Lautstärkesprung (Soft-Takeover greift nach Reconnect, LEDs werden komplett neu gesendet). → M2.
3. **Track lässt sich nicht dekodieren** (kaputte MP3, DRM-AAC, 0-Byte-Datei, Datei während des Spielens gelöscht): Deck bleibt bedienbar, klare Meldung, kein Panik-Abbruch des Loader-Threads. → M1/M5.
4. **Library-Analyse läuft, während live gespielt wird**: kein Dropout bei 256 Frames. → M5 (Xrun-Zähler im Snapshot, Lasttest mit paralleler Analyse).
5. **Ungewöhnliche Tracks**: sehr kurz (< 1 Beat), Stille, kein erkennbares Tempo, Tempo außerhalb 60–200 BPM, Loop über Trackende hinaus, Beat-Jump vor den Anfang. → M5/M7 (Grenzen werden geklemmt, BPM „unbekannt" statt Fantasiewert).

---

## 1. Workspace- und Crate-Struktur

```
rille/
├── Cargo.toml                    # [workspace]: crates/*, src-tauri
├── crates/
│   ├── rille-core/               # gemeinsame Typen ohne Abhängigkeiten zu Audio/MIDI/Tauri
│   ├── rille-engine/             # Echtzeit-Engine (Decks, Mixer, DSP, Routing, Stretch)
│   ├── rille-midi/               # MIDI-I/O, DDJ-200-Mapping, LEDs, Hotplug, Monitor
│   └── rille-library/            # Import, Dekodieren, Analyse, Peaks, SQLite
├── src-tauri/                    # App-Shell: Commands, Bridge, Fenster, Capabilities
└── src/                          # React-Frontend
```

`rille-core` ist zusätzlich zur Vorgabe nötig, damit `rille-midi` Engine-Befehle erzeugen kann, ohne
die Engine zu kennen (keine Abhängigkeit midi → engine). Abhängigkeitsrichtung:

```
rille-core  ←  rille-engine
     ↑      ←  rille-midi
     ↑      ←  rille-library
src-tauri  →  alle vier
```

### rille-core
- `DeckId` (`A`/`B`), `Command` (alle Engine-Befehle, `Copy`, keine Heap-Daten), `Snapshot`
  (Engine-Zustand, `Copy`), `PadMode`, `TempoRange`, `CrossfaderCurve`, `BeatGrid`, `HotCue`.
- Einheiten als Newtypes (`Frames(u64)`, `Bpm(f64)`), damit Beat/Sekunde/Frame nicht verwechselt werden.

### rille-engine
- Kern ist `Engine::process(&mut self, out: &mut [f32], channels: usize)`. Die Methode kennt kein cpal
  und lässt sich deshalb offline im Test rendern.
- Module: `deck` (Transport, Cue-Logik, Loops, Jump, Hot Cues, Reverse, Scratch), `reader`
  (Varispeed-Leser mit Hermite-Interpolation, Rückwärts, Scratch-Geschwindigkeit), `stretch`
  (Signalsmith-Wrapper für Keylock), `mixer` (Kanalfader, Crossfader-Kurven, Gain, Cue-Bus),
  `dsp/eq` (3-Band-Isolator, Kill), `dsp/filter` (bipolares SVF-Filter), `dsp/echo` (Echo-Out),
  `smooth` (Parameter-Glättung), `meter` (Peak/RMS mit Abfall), `sync` (Tempo-/Phasenabgleich),
  `output` (cpal-Streams, Geräteauswahl, Fehlerbehandlung), `cue_out` (zweites Gerät, Drift-Ausgleich).
- Track-Audio kommt als `Arc<TrackAudio>` (f32 interleaved, auf Engine-Rate resampelt) über eine
  Queue. Das alte `Arc` geht über eine Rückgabe-Queue zum Freigeben aus dem Audio-Thread heraus,
  damit der Callback nie dealloziert.

### rille-midi
- `mapping/ddj200.rs`: **deklarative Tabelle** `&[MapEntry]` mit
  `(status, data1, shift_ctx) → Action`, dazu die LED-Tabelle `Feedback → (status, data1)`.
- `decode`: 14-Bit-Paare (MSB puffern, Wert beim LSB übernehmen), relative Jog-Werte (Delta = Wert − 64),
  Tempo-Invertierung, Shift-Tracking (Note 0x3F je Deck, global „irgendein Shift").
- `takeover`: Soft-Takeover je absolutem Regler (Hardware-Wert muss den Software-Wert kreuzen oder
  ihm nahe kommen).
- `led`: diff-basierter LED-Sender (nur Änderungen), Blink-Takt aus der Beat-Phase, Vollabgleich bei Connect.
- `port`: midir-Verbindung, Hotplug per Port-Polling (1 s), Vinyl-Modus-Message beim Connect.
- `monitor`: Ringpuffer der letzten 512 Messages (rein/raus, Zeitstempel) für die Dev-Ansicht.
- `virtual_ddj`: virtueller DDJ-200 (CoreMIDI-Virtual-Source/-Destination) für Tests und
  Entwicklung ohne Hardware.

### rille-library
- `scan` (Ordner rekursiv, Formatfilter), `decode` (symphonia → f32, rubato auf Zielrate), `tags`
  (Titel/Artist/Dauer aus den symphonia-Metadaten), `analysis/bpm` (Onset-Hüllkurve, Tempogramm per
  Autokorrelation, Bereich 60–200 BPM mit Oktav-Prior, konstantes Raster + Phase), `analysis/peaks`
  (3-Band-Peaks low/mid/high in mehreren Auflösungsstufen), `db` (SQLite-Schema, Migrationen),
  `jobs` (Analyse-Warteschlange mit Fortschritt, Worker mit macOS-QoS `utility`).
- Peaks liegen als Binärdatei pro Track im Cache (`~/Library/Caches/io.celox.rille/peaks/<hash>.bin`),
  Metadaten, Raster und Hot Cues in `~/Library/Application Support/io.celox.rille/library.db`.

## 2. Thread-Modell

| Thread | Aufgabe | Darf |
|---|---|---|
| **Main (AppKit/Tauri)** | Fenster, Commands empfangen | alles außer blockieren |
| **Audio Master** (cpal/CoreAudio) | `Engine::process`, Queues leeren, Snapshot schreiben | nur lock-/allokationsfreie Arbeit |
| **Audio Cue** (zweites cpal-Gerät, optional) | liest Cue-Mix aus SPSC-Ring, Drift-Ausgleich über Füllstandsregelung (kleine Resampling-Korrektur) | wie oben |
| **MIDI-In** (midir-Callback) | Bytes → Mapping → `Command` in eigene Engine-Queue | keine Locks auf geteilte UI-Daten |
| **Bridge** (60 Hz) | Snapshot lesen, an Frontend senden, LED-Diff an Controller, Hotplug-Polling, Gerätefehler auswerten, Rückgabe-Queue leeren (Freigaben) | normal |
| **Loader** | Track dekodieren/resampeln (oder aus Cache), `Arc<TrackAudio>` an Engine | normal |
| **Analyse-Worker** (N = Kerne − 2, QoS utility) | BPM, Raster, Peaks, DB-Schreiben | normal, niedrige Priorität |

**Queues in die Engine:** Da `rtrb` nur einen Produzenten erlaubt, bekommt die Engine **zwei
Command-Queues** (eine aus den UI-Commands, eine aus dem MIDI-Thread) und leert beide zu Beginn jedes
Callbacks. Dazu eine Queue für Track-Audio und eine Rückgabe-Queue für alte Buffer.

**Aus der Engine:** Der Snapshot geht über einen lock-freien **Triple Buffer** (immer der neueste,
kein Rückstau). Ereignisse, die nicht verloren gehen dürfen (Xrun, Track-Ende, Gerätefehler), laufen
über eine zusätzliche `rtrb`-Event-Queue.

**Glättung:** Fader, EQ, Filter und Tempo laufen durch One-Pole-Glätter pro Sample bzw. pro
32-Sample-Block (Zipper-frei, in Tests nachgemessen).

## 3. IPC-Konzept Rust ↔ Frontend

- **Commands** (Frontend → Rust): typisierte Tauri-Commands, z. B. `deck_play(deck)`,
  `deck_load(deck, track_id)`, `mixer_set(param, value)`, `library_import(path)`. Sie übersetzen
  in `Command` und schieben in die UI-Queue.
- **Zustand** (Rust → Frontend): ein `tauri::ipc::Channel<StateFrame>` mit maximal 60 Hz. Ein
  `StateFrame` bündelt Decks (Position, Tempo, BPM, Sync, Loop, Cues-Version), Mixer (Fader, EQ,
  Filter), Pegel und Controller-Status. Gesendet wird nur, wenn sich etwas geändert hat oder etwas
  spielt (im Leerlauf 0 Hz).
- **Ereignisse** (selten): Tauri-Events `library-progress`, `device-changed`, `controller-changed`,
  `midi-monitor` (nur bei geöffneter Dev-Ansicht).
- **Waveform-Peaks:** Command `waveform_peaks(track_id, level)` liefert `tauri::ipc::Response` mit
  rohen Bytes → im Frontend `ArrayBuffer`/`Uint8Array`, kein JSON.
- **Flüssige Bewegung:** Das Frontend extrapoliert die Position zwischen zwei 60-Hz-Frames anhand
  von Rate und Zeitstempel. Dadurch laufen die Waveforms mit 120 fps auf ProMotion-Displays.
- **`src/ipc/`** kapselt alles hinter einer Schnittstelle, mit einer **Mock-Implementierung**. Damit
  läuft das Frontend auch im normalen Browser (`pnpm dev:web`) und lässt sich mit Playwright prüfen.

## 4. Frontend-Struktur

```
src/
├── main.tsx · App.tsx
├── design/
│   ├── tokens.css            # Single Source of Truth (generiert + handgepflegte Motion/Shape)
│   ├── tokens.ts             # dieselben Werte typisiert (Springs für motion)
│   ├── seed.ts               # Seed-Color + Deck-Akzente → Script erzeugt tokens.css
│   └── fonts/RobotoFlex.woff2
├── components/               # M3E-Bausteine: Button, IconButton, Fader, Knob, Toggle, Chip, Dialog, Tooltip
├── deck/                     # Deck, PlayButton (Shape-Morph), TempoFader, Pads, TrackInfo, RemainingTime
├── mixer/                    # Channel, EqKnobs, Filter, Crossfader, Meter
├── waveform/                 # Overview (Canvas2D), Detail (WebGL2), Beatgrid-Overlay, Zoom
├── library/                  # Tabelle (virtualisiert), Suche, Sortierung, Drag & Drop, Analyse-Fortschritt
├── settings/                 # Audio, Controller, Darstellung, About-Footer
├── dev/MidiMonitor.tsx
├── keyboard/                 # Shortcut-Map, Hook, Übersicht (`?`)
├── state/                    # Store (useSyncExternalStore) aus StateFrames, Extrapolation
└── ipc/                      # tauri.ts · mock.ts · types.ts
```

Die Farbtokens kommen aus `@material/material-color-utilities` (Apache-2.0) und werden **zur
Build-Zeit** erzeugt, nicht zur Laufzeit. Es ist eine einzige kleine Abhängigkeit, keine UI-Library.

## 5. Meilensteine

Jeder Meilenstein endet mit einem lauffähigen Stand, grünen Tests (`cargo test`, `cargo clippy`,
`pnpm lint`, `pnpm typecheck`, `pnpm test`), einem Commit und einem Push.

### M0 – Fundament
Git-Repo + öffentliches GitHub-Repo, MIT-LICENSE, Cargo-Workspace mit leeren Crates, Tauri-2-App mit
Titlebar Overlay + Vibrancy, Vite/React/TS strict, ESLint/Prettier/Vitest, Token-Generator +
Roboto Flex, CSP und minimale Capabilities, GitHub-Actions-CI (macOS-Runner), `CLAUDE.md` und die drei
Rules-Dateien, README-Gerüst.
**Fertig, wenn:** `pnpm tauri dev` ein leeres, gestaltetes Fenster öffnet und die CI grün ist.

### M1 – Engine-Kern: Wiedergabe und Mixer-Grundlage
cpal-Ausgabe mit Geräteauswahl, Buffergröße und Latenzanzeige (Puffer + gemeldete Gerätelatenz), Command-Queues, Triple-Buffer-Snapshot, Loader
(symphonia + rubato), Deck-Transport (Play/Pause, Pioneer-Cue, Shift+Cue → Anfang, Seek), Kanalfader,
Crossfader mit Kurven, Pegel, Glättung, `assert_no_alloc`, Xrun-Zähler, Geräteausfall ohne Absturz.
Minimal-UI: Datei aufs Deck ziehen, abspielen, Fader bedienen.
**Tests:** Cue-Logik als Zustandsmaschine, Crossfader-Kurven (Summenpegel, Endpunkte), Glättung
ohne Sprung > ε, Offline-Rendering bekannter Signale, Allokationsfreiheit des Callbacks.

### M2 – Controller DDJ-200
`rille-midi` vollständig: Mapping-Tabelle, 14-Bit, Jog-Deltas, Shift-Tracking, Tempo-Invertierung,
Soft-Takeover, Hotplug, LED-Diff und Vollabgleich, Loaded-LED, Vinyl-Modus-Einstellung, virtueller
DDJ-200, MIDI-Monitor-Ansicht, Controller-Status in der UI.
**Tests:** jede Tabellenzeile des PDFs als Testfall (eingehend und LED ausgehend), Reihenfolge
MSB/LSB, LSB ohne MSB, Shift-Wechsel mitten in der Bewegung, Takeover nach Reconnect.
**Hardware-Abnahme 1** mit angeschlossenem Gerät: Kanal 7 statt 5 bestätigen, Loaded-LED-Aus,
CC des Kanalfaders bei Shift, Fader Start am Crossfader.

### M3 – Mixer-DSP und Effekte
3-Band-EQ (Isolator, Linkwitz-Riley-Weichen) mit Kill, bipolares Filter (Lowpass ↔ neutral ↔ Highpass,
Totzone in der Mitte, resonanzbegrenzt), Echo-Out als Transition FX mit wählbarem Effekt, Gain.
**Tests:** Frequenzgang an Stützstellen, Kill dämpft ≥ 60 dB, Filter-Mitte ist bitgenau neutral,
Echo klingt aus, keine Denormals.

### M4 – Tempo, Keylock, Scratch
Tempo-Fader mit Bereichen ±6/±10/±16 %/Wide, Varispeed, Keylock über Signalsmith, Pitch-Bend über
den Jog-Rand, Scratch (Vinyl-Modus + Jog-Touch, geglättete Geschwindigkeit, Rücksprung zur Rate nach
dem Loslassen), Shift+Teller-Suchlauf, Reverse (solange gehalten), Fader Start.
**Tests:** Tempo-Mapping inkl. Invertierung, Keylock hält die Tonhöhe (FFT-Peak bleibt ±1 %),
Scratch ist allokationsfrei, Reverse kehrt an der richtigen Stelle zurück.

### M5 – Library und Analyse
Ordner importieren, SQLite, Tags, Analyse-Warteschlange mit Fortschritt (BPM, Raster, Peaks),
Suche, Sortierung (BPM/Titel/Artist/Dauer), virtualisierte Liste, Drag & Drop aufs Deck,
Shift+Jog-Rand zur Navigation, Shift+Kopfhörer-Cue lädt. Laden analysierter Tracks unter 300 ms.
**Tests:** BPM-Erkennung auf synthetischen Tracks (Klicks bei 90/120/128/174 BPM, mit Swing und
Pausen), Raster-Phase ±5 ms, Peaks-Format, DB-Migration, kaputte und leere Dateien. Lasttest:
Analyse parallel zum Abspielen ohne Xrun.

### M6 – Waveforms und Deck-Oberfläche
Übersicht (Canvas2D) und Detailansicht (WebGL2) mit Frequenzband-Farben, Beatgrid, Zoom,
Extrapolation auf 120 fps, Leerlauf ohne Rendering. Deck-UI in M3E: Play-Button-Morph, große
tabellarische Ziffern, Restzeit-Warnung (letzte 30 s), Shared Element Transition beim Laden,
Controller-Spiegelung (Bewegung am Gerät sofort sichtbar).
**Tests:** Vitest für Extrapolation und Zoom-Mathematik; Playwright gegen das Mock-Backend
(Frame-Zeiten, kein Rendering im Leerlauf, Reduced Motion).

### M7 – Beat-Funktionen und Pads
Sync (einmalig) und Sync-Lock (Tempo + Phase), Hot Cues (8, persistent), Beat Loops (1/4 bis 32),
Beat Jump (±1/2 bis ±16), Pad-Modi (umschaltbar in App und per Tastatur), Pad-LEDs mit Blinken,
Beatgrid manuell korrigieren (verschieben, BPM anpassen, Tap).
**Tests:** Loop-Grenzen (über das Trackende, Loop beim Jump), Sync mit halbem/doppeltem Tempo,
Hot-Cue-Persistenz, Pad-LED-Zustände.
**Hardware-Abnahme 2:** ein komplettes Übergangs-Set am Gerät.

### M8 – Kopfhörer-Routing
Kopfhörer-Cue je Kanal, Master Cue, Cue/Master-Mix und Cue-Lautstärke, zweites Gerät mit
Drift-Ausgleich oder 4-Kanal-Gerät (1/2 Master, 3/4 Cue), Gerätewechsel im Betrieb.
**Tests:** Cue-Bus-Mischung, Drift-Regler hält den Füllstand stabil (simulierte ±200 ppm),
Abziehen des Cue-Geräts lässt den Master weiterlaufen.

### M9 – Feinschliff und Abnahme
Vollständige Tastatursteuerung + `?`-Übersicht, Leerzustände (keine Tracks, kein Controller, kein
Gerät), Einstellungen (Vinyl-Modus, Tempo-Bereich, Crossfader-Kurve, Puffer, Theme), Light Theme,
About-Dialog mit Footer, README mit Architektur und Controller-Belegung, Messung aller
Performance-Budgets mit protokollierten Zahlen.

## 6. Risiken

1. **Signalsmith im Callback:** Ob `process()` nach der Konfiguration wirklich nicht alloziert, wird
   in M4 zuerst geprüft (`assert_no_alloc`-Test). Sonst bleibt nur ein eigener Stretch-Thread mit
   Vorpuffer, was Latenz kostet.
2. **Speicher:** Vollständig dekodierte Tracks belegen als f32 bei 48 kHz rund 23 MB pro Minute.
   Zwei Decks mit je 10 Minuten sind gut 460 MB; das ist vertretbar, wird aber gemessen. Notfalls
   werden die Buffer als i16 gehalten.
3. **Zwei Geräte für Master und Cue** laufen auf unterschiedlichen Takten. Ohne Drift-Ausgleich gibt
   es nach Minuten Knackser; das deckt M8 mit einem eigenen Test ab.
4. **BPM-Erkennung** ohne Fremdbibliothek: Genauigkeit wird gegen synthetische Tracks und eine
   kleine echte Referenzliste gemessen, Oktavfehler sind die erwartete Hauptquelle.
5. **Hotplug-Erkennung** über Polling statt CoreMIDI-Notifications: 1 s Verzögerung, dafür ohne
   eigenen CoreMIDI-Code. Reicht nach meiner Einschätzung.
