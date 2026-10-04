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
pnpm badges                  # README-Zahlen neu messen (.github/badges/*.json)
pnpm screenshots             # docs/screenshots/* und docs/banner.png aus der Browser-Demo rendern
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
- **CoreMIDI sieht neue Geräte nur, wenn der erste MIDI-Client des Prozesses auf einem Thread
  mit Run-Loop entstand.** `rille_midi::port::init_on_main_thread()` läuft deshalb im
  Tauri-`setup` (Hauptthread). Ohne das findet das Hotplug-Polling einen später angesteckten
  DDJ-200 nie (per Gegenprobe belegt).
- Ohne Hardware testen: `cargo run -p rille-midi --example virtual_ddj` startet einen
  virtuellen DDJ-200 neben der laufenden App; `--example list_ports` listet alle MIDI-Ports.
- cpal ruft den Fehler-Callback teils auf dem **Render-Thread** auf und meldet jede
  CoreAudio-Überlastung als `ErrorKind::Xrun` – dort nur Atomics anfassen
  (`StreamStats::record_error`), Überlastungen zählen statt den Stream neu aufzubauen.
- **Signalsmith Stretch braucht beim `seek` einen ganzen Analyseblock Vorlauf**
  (≈ `input_latency × 2` Frames), nicht nur einen Audioblock. Mit zu kurzem Vorlauf lag nach
  jedem Cue-Sprung ~30 ms Stille *hinter* der Varispeed-Brücke. Eigener Puffer `preroll`, in
  `Player::new` alloziert. Tests zu Lücken müssen länger rendern als Brücke + Überblendung
  (`output_latency + 256` Frames) – der erste Test mit 50 ms war dafür blind.
- `assert_no_alloc` sieht C++ nicht. Signalsmith wird über den macOS-`malloc_logger`-Haken
  geprüft (`tests/stretch_alloc.rs`); bei einem Update der Crate diesen Test zuerst laufen lassen.
- `JOG_TICKS_PER_REV` (player.rs) ist eine Annahme und wird bei der Hardware-Abnahme kalibriert:
  eine Tellerumdrehung muss so weit spulen wie eine Umdrehung einer Platte bei 33⅓ U/min.
- **Keylock verlassen = Stretcher ausblenden, nicht abschneiden.** Sein Signal ist nicht
  sampelgleich mit dem Track (Phasenvocoder); ein harter Wechsel auf die Rohdaten klickt.
  `Player::leave_keylock` lässt ihn 4 ms weiterlaufen; solange er spielt oder ausblendet
  (`owns_fade_out`), setzt die Engine weder Rohdaten-Tail noch Fade-in.
- **Signalsmith würfelt:** über Streckfaktor 2 randomisiert es Phasen, Seed aus
  `std::random_device`. Beim Hochlaufen der Rate aus ~0 (nach Reverse/Scratch) wurde die
  Ausrichtung so zur Glückssache (Testergebnisse schwankten zwischen Läufen). Keylock greift
  deshalb erst, wenn die Rate eingeschwungen ist (`rate_settled`).
- **Testsignale mit Bedacht:** reiner Sinus versteckt Keylock-Klicks (der Vocoder trifft ihn
  fast exakt) → `rich_track`; Sprünge um volle Sekunden landen bei 110/523/1871 Hz auf derselben
  Phase und verstecken Sprungklicks → krumme Sprungweiten; periodische Klicks machen
  Korrelationen mehrdeutig → unregelmäßige Abstände, Suchfenster < halbe Periode.
- Tempo-Fader: in der App ist der Fader-Wert oben = 1 (wie alle Fader), Tempo oben = −1
  (langsamer, wie aufgedruckt). Umrechnung nur über `faderToTempo`/`tempoToFader`
  (`src/state/extrapolate.ts`) bzw. `tempo_to_fader` (dto.rs) – sonst kippt die Richtung.

## Doku, Badges, Screenshots
- README ist zweisprachig: `README.md` (Englisch, vollständige Badge-Liste) und `README.de.md`.
  Inhaltliche Änderungen in **beiden** pflegen; `tools/readme.test.mjs` prüft Bilder,
  Badge-Dateien, Spenden-/Bewerten-Links und den Changelog-Eintrag der aktuellen Version.
- Version steht dreimal (Cargo-Workspace, `package.json`, `tauri.conf.json`) – ein Test
  erzwingt Gleichstand. Release = alle drei anheben + `CHANGELOG.md` + Tag `vX.Y.Z`.
- Die großen Badges (Version, Unit-Tests, LoC, Testcode) kommen aus `.github/badges/*.json`
  (shields.io-Endpoint). `tools/stats.mjs` misst sie; die CI committet sie bei jedem Push auf
  `main` als `chore: refresh badge counters [skip ci]`. **Deshalb vor jedem Push
  `git pull --rebase`**, sonst wird der Push abgewiesen.
- Screenshots entstehen aus der Browser-Demo (`?demo`: zwei spielende Decks, verbundener
  Controller, simulierter MIDI-Verkehr) im macOS-Fensterrahmen `tools/frame.html`; das Banner
  aus `tools/banner.html` (1280×640, zugleich GitHub-Social-Preview). Nach sichtbaren
  UI-Änderungen `pnpm screenshots` laufen lassen.
- Social Preview lässt sich nicht per API setzen: GitHub → Settings → Social preview →
  `docs/banner.png` hochladen.
