# rille

Native DJ-App für macOS mit Unterstützung für den **Pioneer DDJ-200**. Gebaut mit Tauri 2,
einer Echtzeit-Audio-Engine in Rust und einer React-Oberfläche nach Material 3 Expressive.
Voll bedienbar auch ohne Controller, per Maus und Tastatur.

> Status: in Entwicklung (M1 – Engine-Kern). Zwei Decks spielen Dateien aus dem Finder, mit
> Cue, Kanalfadern, Crossfader und Pegeln. Controller, Library und Waveforms folgen.

## Voraussetzungen

- macOS 13 oder neuer (Apple Silicon empfohlen)
- Rust (stable) und Xcode Command Line Tools
- Node.js 22 und pnpm 10

## Entwicklung

```bash
pnpm install
pnpm tauri dev      # App starten
pnpm check          # alle Tests und Linter
pnpm dev            # Oberfläche im Browser mit simuliertem Backend
pnpm e2e            # Browser-Tests
```

## Architektur

```
React-Oberfläche  ──Commands──▶  src-tauri  ──rtrb──▶  rille-engine (CoreAudio-Callback)
       ▲                            │                        │
       └──── 60-Hz-Snapshots ◀──── Bridge ◀── Triple Buffer ─┘
                                    │
DDJ-200 ◀──LEDs── rille-midi ──Commands──▶ Engine
                     ▲
                 USB-MIDI
rille-library: Import, Dekodieren, BPM/Beatgrid, Waveform-Peaks, SQLite
```

Details: [`CLAUDE.md`](CLAUDE.md) und der [Masterplan](docs/superpowers/plans/2026-10-04-rille-masterplan.md).

## Controller-Belegung

Die vollständige Belegungstabelle folgt mit Meilenstein M2.

Grundlage ist die offizielle *DDJ-200 List of MIDI messages* von Pioneer DJ (Download auf der
Support-Seite des DDJ-200). Das PDF liegt aus Urheberrechtsgründen nicht im Repo; für die
Entwicklung nach `docs/ddj-200_midi_message_list.pdf` legen.

## Lizenz

MIT – siehe [LICENSE](LICENSE). © Martin Pfeffer | [celox.io](https://celox.io)
