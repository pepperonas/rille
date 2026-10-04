# rille

Native DJ-App für macOS mit Unterstützung für den **Pioneer DDJ-200**. Gebaut mit Tauri 2,
einer Echtzeit-Audio-Engine in Rust und einer React-Oberfläche nach Material 3 Expressive.
Voll bedienbar auch ohne Controller, per Maus und Tastatur.

> Status: in Entwicklung (M2 – Controller). Zwei Decks spielen Dateien aus dem Finder, mit Cue,
> Kanalfadern, Crossfader und Pegeln, bedienbar per Maus, Tastatur und DDJ-200. EQ, Tempo,
> Library und Waveforms folgen.

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

## Controller-Belegung (Pioneer DDJ-200)

rille erkennt den DDJ-200 automatisch beim Anstecken. Status und Vinyl-Modus stehen in den
Einstellungen; der MIDI-Monitor (⌘⌥M) zeigt jede Nachricht live.

| Element | Funktion | Mit Shift | Status |
|---|---|---|---|
| Play/Pause | Start/Stop | Rückwärts, solange gehalten | Play ✓ · Reverse folgt (M4) |
| Cue | Pioneer-Cue (setzen, Vorhören, zurück) | Zum Trackanfang | ✓ |
| Kanalfader | Lautstärke | Fader Start (hoch = Play, 0 = zurück zum Cue) | ✓ |
| Crossfader | Überblenden (Kurve in der App) | – | ✓ |
| Jog berühren + Teller | Scratch (Vinyl-Modus) | Schneller Suchlauf | M4 |
| Jog-Rand | Pitch-Bend | Library-Navigation | M4 / M5 |
| Tempo-Fader | Tempo (oben langsamer) | – | M4 |
| Beat Sync | kurz: Sync, lang: Sync-Lock | Tempo-Bereich wechseln | M7 |
| EQ Hi/Mid/Low | 3-Band-EQ | – | M3 |
| CFX | Bipolares Filter (links Lowpass, rechts Highpass) | – | M3 |
| Kopfhörer-Cue | Kanal vorhören | Markierten Track laden | M8 / M5 |
| Master Cue | Master im Kopfhörer | – | M8 |
| Transition FX | Echo-Out aufs aktive Deck | Effekt wechseln | M3 |
| Pads 1–8 | Hot Cue / Beat Loop / Beat Jump | löschen / Loop verlassen | M7 |

LEDs: Play blinkt bei geladenem, pausiertem Deck; Cue blinkt, wenn das Deck abseits des
Cue-Punkts steht; „Loaded“ leuchtet bei geladenem Track. Alle analogen Regler haben
Soft-Takeover: Nach dem Wiederanstecken (oder einer Änderung in der App) übernimmt ein Regler
erst, wenn er die aktuelle Stellung erreicht – nichts springt.

Ohne Hardware: `cargo run -p rille-midi --example virtual_ddj` startet einen virtuellen
DDJ-200 neben der laufenden App.

Grundlage ist die offizielle *DDJ-200 List of MIDI messages* von Pioneer DJ (Download auf der
Support-Seite des DDJ-200). Das PDF liegt aus Urheberrechtsgründen nicht im Repo; für die
Entwicklung nach `docs/ddj-200_midi_message_list.pdf` legen.

## Lizenz

MIT – siehe [LICENSE](LICENSE). © Martin Pfeffer | [celox.io](https://celox.io)
