# Performance-Protokoll

Jedes Budget aus dem Masterplan mit Messweg, Datum und Ergebnis. Neue Messungen werden **oben**
angefügt; ältere bleiben stehen, damit Regressionen sichtbar sind.

## 2026-10-11 – M9-Abnahme

**Maschine:** MacBook Pro, Apple M1 Pro (10 Kerne), macOS 27. **Fremdlast** während der Messung:
Load Average 14–17 (andere Sitzungen kompilierten parallel) – die Werte sind damit eher
pessimistisch. Release-Builds.

| Budget | Ziel | Ergebnis | Messweg |
|---|---|---|---|
| Puffer | 256 Frames @ 48 kHz ohne Aussetzer | 0 von 5.625 Blöcken über Budget (30 s, zwei Decks mit Keylock, Echtzeit-Priorität); langsamster Block 3,4 ms = 64 % von 5,3 ms unter Fremdlast | `cargo run --release -p rille-library --example load_test -- ~/Music/DJ` |
| Abspielen während der Analyse | kein Block über Budget | 8 Analyse-Worker, 797 Analysen in 30 s: 0 von 5.625 Blöcken über Budget, langsamster 0,87 ms = 16 % | dto. |
| Engine-Last, Keylock aus / an | gering | 0,4 % / 2,0 % eines Kerns im Mittel (256 Frames) | `cargo run --release -p rille-engine --example bench_keylock` |
| Langsamster Block mit Keylock | innerhalb der Blockzeit | 692 µs = 13 % bei 256 Frames, 673 µs = 25 % bei 128, 808 µs = 61 % bei 64 | dto. |
| Track laden | < 300 ms bis spielbar | spielbar nach 0,9–1,8 ms, erste Sekunde nach 3,5–3,9 ms; vollständig dekodiert nach 520 ms (4:45-MP3) bzw. 720 ms (6:50-MP3) im Hintergrund | `cargo run --release -p rille-library --example decode -- <datei>` |
| Analyse eines Tracks | im Hintergrund | 0,96 s für eine 6:50-MP3 auf einem Kern (BPM 121,99, Konsistenz 86 %) | `cargo run --release -p rille-library --example analyze -- <datei>` |
| Controller → Aktion | < 10 ms bis hörbar (ohne Gerätelatenz) | MIDI bis dekodierte Aktion: Median 43 µs, p99 83 µs, schlechtester von 1000: 1,0 ms (virtueller Port über CoreMIDI). Hörbar ab dem nächsten Audioblock: + höchstens 5,3 ms ⇒ < 6,4 ms | `cargo run --release -p rille-midi --example midi_latency` |
| Oberfläche, zwei Decks spielen | flüssig, keine langen Tasks | 60,2 fps, Bildabstand Median 16,7 ms / p99 16,8 ms / schlechtester 16,8 ms, 0 Long Tasks, 120 Waveform-Zeichnungen/s (2 Decks × 60) | `node tools/perf-ui.mjs` (Browser-Demo, 1440×900) |
| Oberfläche im Stand | nichts zeichnen | 0 Waveform-Zeichnungen/s, 0 Long Tasks | dto. |
| Hauptthread der Oberfläche | gering | 24 % eines Kerns mit zwei spielenden Decks, 17 % im Stand – **im Browser läuft das Mock-Backend (Decks, Pegel, MIDI-Simulation) im selben Thread**; der Anteil fürs Zeichnen beider Decks ist die Differenz, ≈ 8 % | dto. |
| Leerlauf der App | minimal | 0,0–0,8 % eines Kerns (Release, Ausgabe offen, nichts geladen, Controller-Suche aktiv), 85–100 MB RSS des App-Prozesses; der WebView läuft in einem eigenen WebKit-Prozess | `ps`/`top -pid` 20 s nach dem Start |
| Speicher je Track | vertretbar | 48 kHz × 2 Kanäle × 4 Byte = 23 MB je Minute (gerechnet; zwei 10-Minuten-Tracks ≈ 460 MB) | – |

**Headless-Hinweis:** Chrome begrenzt im Headless-Betrieb auf 60 Hz. Auf ProMotion-Displays
fährt die Oberfläche bis 120 Hz; die Fortschreibung zwischen den 60-Hz-Frames der Engine
(`extrapolate.ts`) ist dafür ausgelegt.
