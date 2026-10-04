# CLAUDE.md – rille

Native DJ-App für macOS (Tauri 2 + Rust + React 19/TypeScript), primärer Controller Pioneer
DDJ-200. MIT, öffentliches Repo `pepperonas/rille`. Masterplan mit Meilensteinen:
`docs/superpowers/plans/2026-10-04-rille-masterplan.md` (vor jedem Meilenstein ein eigener
Teilplan im selben Ordner).

## Befehle

```bash
pnpm install                 # Frontend-Abhängigkeiten (inkl. lokaler tauri-cli)
pnpm tauri dev               # App starten (Vite auf :1420 + Rust)
pnpm dev                     # nur Frontend im Browser (Mock-Backend simuliert zwei Decks)
pnpm e2e                     # Playwright gegen das Mock-Backend (System-Chrome)
pnpm tokens                  # Farb-Tokens neu erzeugen nach Seed-Änderung
pnpm check                   # alles: typecheck, lint, vitest, rustfmt, clippy, cargo test
pnpm tauri build             # .app/.dmg (unsigniert)
```

## Struktur

| Pfad | Zweck |
|---|---|
| `crates/rille-core` | gemeinsame Typen (DeckId, Commands, Snapshot), keine Audio/MIDI/Tauri-Abhängigkeit |
| `crates/rille-engine` | Echtzeit-Engine; Callback allokations- und lockfrei |
| `crates/rille-midi` | MIDI-I/O, deklaratives DDJ-200-Mapping, LEDs, Hotplug, Monitor |
| `crates/rille-library` | Import, Dekodieren, BPM/Beatgrid, Waveform-Peaks, SQLite |
| `src-tauri` | App-Shell, Commands, 60-Hz-Bridge, Fenster (Overlay + Vibrancy) |
| `src/design` | Design-Tokens (Single Source of Truth) |
| `scripts/gen-tokens.mjs` | erzeugt `src/design/color.generated.css` aus den Seeds |
| `docs/ddj-200_midi_message_list.pdf` | einzige Quelle fürs Controller-Mapping |

## Architektur in einem Absatz
Engine rendert in `Engine::process` (ohne cpal testbar). UI und MIDI schicken `Command`s über je
eine `rtrb`-Queue; die Engine meldet ihren Zustand über einen Triple Buffer, eine Bridge sendet
ihn mit max. 60 Hz per Tauri Channel ans Frontend und als LED-Diff an den Controller. Dekodieren
und Analyse laufen in Workern. Waveform-Peaks gehen als Binärdaten (`tauri::ipc::Response`).
Das Frontend rechnet nichts Audio-Relevantes.

## Konventionen
- Regeln: `.claude/rules/audio-realtime.md`, `.claude/rules/midi-mapping.md`,
  `.claude/rules/frontend-m3e.md` – vor Arbeit im jeweiligen Bereich lesen.
- Kein `unwrap`/`expect`/`panic!` außerhalb von Tests (Workspace-Lints, `clippy.toml`).
- Commits: Conventional Commits, Englisch. UI-Texte Deutsch.
- Nach jedem Meilenstein: `pnpm check` grün, Commit, Push, frischer Reviewer.

## Fallstricke
- `@material/material-color-utilities@0.4.0` hat kaputte ESM-Importe (fehlendes `.js`); behoben
  per `pnpm patch` (`patches/`). Beim Update der Version prüfen, ob der Patch noch nötig ist.
- TypeScript bleibt auf 6.0.x, solange `typescript-eslint` TS 7 nicht unterstützt (Peer `<6.1`).
- Das Fenster ist transparent (Vibrancy) und erzwingt `theme: Dark`; ohne das zieht der
  Vibrancy-Grund das helle System-Theme durch.
- Screenshot des Fensters: `kCGWindowName == 'rille'` suchen (Tauri legt mehrere unsichtbare
  Hilfsfenster an), dann `screencapture -l <id>`.
- **Integrationstests/Beispiele** (`tests/*.rs`, `examples/*.rs`) gelten für Clippy nicht als
  Testcode: dort `#![allow(clippy::unwrap_used, ...)]` auf Dateiebene.
- Clippy 1.98 verlangt `as_chunks::<N>()` statt `chunks_exact(N)` bei konstanter Größe.
- React StrictMode ruft Effekte doppelt auf: Abmeldungen dürfen nur den **eigenen** Empfänger
  entfernen (siehe `mock.ts`), sonst killt die verworfene erste Verbindung die zweite.
- `assert_no_alloc` im Test mit Feature `warn_debug` (zählt statt abzubrechen); die App selbst
  bricht im Debug-Build bei einer Allokation im Callback ab. Gegenprobe steht im Test.
- Engine-Rate wird einmal beim Start gewählt (48 kHz bevorzugt, sonst 44,1 kHz); Geräte ohne
  diese Rate melden einen klaren Fehler statt die Engine neu zu bauen.
- Fenster-Screenshot: liegt rille hinter einem anderen Fenster oder auf dem zweiten Monitor,
  scheitert `screencapture -l`; dann nicht fremde Fenster abfotografieren, sondern über das
  Mock-Frontend im Browser prüfen.
- Dekodieren einer 3:43-MP3 dauert ~360 ms (Release) – für die 300-ms-Ladezeit braucht M5 einen
  PCM-Cache.
