<div align="center">

<a href="https://github.com/pepperonas/rille"><img src="docs/banner.png" alt="rille — native DJ-App für macOS, gemacht für den Pioneer DDJ-200" width="100%"></a>

# 🎛️ rille

[English](README.md) · **Deutsch**

**Eine native DJ-App für macOS mit Echtzeit-Audio-Engine in Rust, gebaut um den Pioneer DDJ-200 — und vollständig mit Maus und Tastatur spielbar.**

<p>
  <a href="https://www.paypal.com/donate/?business=martin.pfeffer@celox.io&currency_code=EUR&item_name=rille"><img alt="Spenden via PayPal" height="46" src="https://img.shields.io/badge/%E2%98%95_Kaffeekasse-Spenden_via_PayPal-00457C?style=for-the-badge&logo=paypal&logoColor=white"></a>
  &nbsp;
  <a href="https://g.page/r/CXgdRV3QysvxEBM/review"><img alt="celox.io auf Google Maps bewerten" height="46" src="https://img.shields.io/badge/%E2%AD%90_celox.io_bewerten-auf_Google_Maps-4285F4?style=for-the-badge&logo=googlemaps&logoColor=white"></a>
</p>

<!-- BADGES:BIG — berechnet von tools/stats.mjs, bei jedem Push auf main von der CI erneuert. -->
[![version](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/pepperonas/rille/main/.github/badges/version.json&style=for-the-badge)](CHANGELOG.md)
[![unit tests](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/pepperonas/rille/main/.github/badges/unit-tests.json&style=for-the-badge)](#-tests)
[![lines of code](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/pepperonas/rille/main/.github/badges/loc.json&style=for-the-badge)](#%EF%B8%8F-projektstruktur)
[![test code](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/pepperonas/rille/main/.github/badges/test-code.json&style=for-the-badge)](#-tests)

</div>

[![Rust tests](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/pepperonas/rille/main/.github/badges/tests-rust.json)](crates)
[![frontend tests](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/pepperonas/rille/main/.github/badges/tests-frontend.json)](src)
[![e2e tests](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/pepperonas/rille/main/.github/badges/tests-e2e.json)](e2e)
[![Rust LoC](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/pepperonas/rille/main/.github/badges/loc-rust.json)](crates)
[![TypeScript LoC](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/pepperonas/rille/main/.github/badges/loc-ts.json)](src)
[![CSS LoC](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/pepperonas/rille/main/.github/badges/loc-css.json)](src)
[![CI](https://img.shields.io/github/actions/workflow/status/pepperonas/rille/ci.yml?branch=main&label=CI&logo=githubactions&logoColor=white)](https://github.com/pepperonas/rille/actions/workflows/ci.yml)
[![Latest tag](https://img.shields.io/github/v/tag/pepperonas/rille?sort=semver&label=tag&logo=github&logoColor=white&color=5B4BFF)](https://github.com/pepperonas/rille/tags)
[![Last commit](https://img.shields.io/github/last-commit/pepperonas/rille?logo=git&logoColor=white)](https://github.com/pepperonas/rille/commits/main)
[![License: MIT](https://img.shields.io/github/license/pepperonas/rille?color=blue)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%2013%2B-000000?logo=apple&logoColor=white)](#-loslegen)
[![Rust](https://img.shields.io/badge/Rust-2024%20edition%20%C2%B7%201.98-DEA584?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![Tauri](https://img.shields.io/badge/Tauri-2.12-24C8DB?logo=tauri&logoColor=white)](https://tauri.app)
[![React](https://img.shields.io/badge/React-19-61DAFB?logo=react&logoColor=black)](https://react.dev)
[![Material 3 Expressive](https://img.shields.io/badge/Material%203-Expressive-6750A4?logo=materialdesign&logoColor=white)](https://m3.material.io)
[![Controller](https://img.shields.io/badge/controller-Pioneer%20DDJ--200-E53935)](#%EF%B8%8F-pioneer-ddj-200)
[![Realtime safe](https://img.shields.io/badge/Audio--Callback-allokationsfrei%20(getestet)-2E9E5B?logo=rust&logoColor=white)](#-audio-engine)
[![Offline](https://img.shields.io/badge/Netzwerk-keins-2E7D32)](#-design)
[![Made by celox.io](https://img.shields.io/badge/made%20by-celox.io-5B4BFF)](https://celox.io)

<sub>Die vollständige Badge-Übersicht steht im [englischen README](README.md).</sub>

> **Stand: frühe Entwicklung (v0.1.0).** Zwei Decks spielen Dateien aus dem Finder, mit Cue nach
> Pioneer-Art, Isolator-EQ, Filter, Transition-FX, Kanalfadern, Crossfader und Pegeln — bedienbar
> mit Maus, Tastatur und DDJ-200. Tempo/Keylock, Library, Waveforms und Beat-Funktionen folgen Meilenstein für
> Meilenstein (siehe [Roadmap](#%EF%B8%8F-roadmap)). Ein fertiges Release gibt es noch nicht.

---

## Inhalt

- [📸 Screenshots](#-screenshots)
- [✨ Funktionen](#-funktionen)
- [🎛️ Pioneer DDJ-200](#%EF%B8%8F-pioneer-ddj-200)
- [⌨️ Maus und Tastatur](#%EF%B8%8F-maus-und-tastatur)
- [🚀 Loslegen](#-loslegen)
- [🔊 Audio-Engine](#-audio-engine)
- [🏛️ Architektur](#%EF%B8%8F-architektur)
- [🎨 Design](#-design)
- [📏 Performance](#-performance)
- [🧪 Tests](#-tests)
- [🗂️ Projektstruktur](#%EF%B8%8F-projektstruktur)
- [🗺️ Roadmap](#%EF%B8%8F-roadmap)
- [🧯 Fehlerbehebung](#-fehlerbehebung)
- [🤝 Mitmachen](#-mitmachen)
- [❤️ Unterstützen](#%EF%B8%8F-unterstützen)
- [📄 Lizenz](#-lizenz)

---

## 📸 Screenshots

<p align="center">
  <img src="docs/screenshots/decks.png" alt="Zwei spielende Decks: Titel, Zeit und Restzeit, Transport, Mixer mit Kanalfadern und Pegeln" width="100%">
</p>

| Erster Start | Einstellungen | MIDI-Monitor |
|:---:|:---:|:---:|
| <img src="docs/screenshots/empty.png" alt="Leere Decks und Library erklären den nächsten Schritt" width="100%"> | <img src="docs/screenshots/settings.png" alt="Einstellungen: Ausgabegerät, Puffer, Latenz, Aussetzer, Controller, Vinyl-Modus" width="100%"> | <img src="docs/screenshots/midi-monitor.png" alt="MIDI-Monitor mit dekodierten Controller-Nachrichten" width="100%"> |
| Leerzustände sagen, was als Nächstes zu tun ist | Gerät, Puffer, Latenz, Aussetzer, Controller | Jede MIDI-Nachricht, dekodiert, live (⌘⌥M) |

<sub>Die Screenshots entstehen aus der Browser-Demo (`pnpm screenshots`) und sind damit ohne
Hardware reproduzierbar. Der MIDI-Verkehr im Monitor-Screenshot ist simuliert.</sub>

## ✨ Funktionen

**Schon da**

- **Zwei Decks** — Audiodatei aus dem Finder auf ein Deck ziehen; Titel, Zeit und Restzeit
  (tabellarische Ziffern, aus zwei Metern lesbar), Fortschrittslinie, Auswerfen.
- **Cue nach Pioneer-Art** — pausiert: Cue-Punkt setzen; auf dem Cue-Punkt halten = vorhören,
  loslassen = zurück; Play beim Halten = weiterspielen; spielend: zurück zum Cue und Pause.
  Shift + Cue springt an den Anfang.
- **Mixer** — Kanalfader mit DJ-Kennlinie, Master, Crossfader mit drei Kurven (Blend =
  konstante Leistung, Linear, Cut zum Scratchen), Segment-Pegel pro Kanal und Master.
- **Isolator-EQ** — Hi / Mid / Low pro Kanal mit Linkwitz-Riley-Weichen bei 200 Hz und 2 kHz
  (neutral flach); Regler ganz links oder Klick auf den Bandnamen = Kill, bis +6 dB Anhebung.
- **Bipolares Filter** — ein Regler pro Kanal: links schließt ein Tiefpass, rechts öffnet ein
  Hochpass, in der Mitte echter Bypass.
- **Transition-FX** — eine Taste nimmt das lautere spielende Deck aus dem Mix: *Echo-Out* (das
  Deck blendet aus, sein letzter Moment hallt nach) oder *Filter-Out* (ein Hochpass fährt hoch).
  Danach pausiert das Deck; nochmal drücken bricht ab.
- **Klickfreier Transport** — Stopp, Start, Cue-Sprünge und Seeks blenden über 4 ms statt die
  Wellenform abzuschneiden.
- **Audio-Einstellungen** — Ausgabegerät, Puffer 64–2048, gemessene Latenz, Aussetzer-Zähler;
  ein abgezogenes Interface wechselt aufs Standardgerät, ohne das Set zu stoppen.
- **Formate** — MP3, AAC/M4A, ALAC, FLAC, WAV, AIFF; beim Laden auf die Engine-Rate resampelt.
- **Pioneer DDJ-200** — anstecken und loslegen, LED-Rückmeldung, Soft-Takeover, Vinyl-Modus,
  MIDI-Monitor (Details [unten](#%EF%B8%8F-pioneer-ddj-200)).
- **Oberfläche in Material 3 Expressive** — dunkel zuerst, eigene Akzentfarbe pro Deck,
  Spring-Animationen, der Play-Knopf morpht zwischen Kreis und abgerundetem Quadrat.
- **„Über rille“** — Version, Spenden- und Bewerten-Button, Link zum Quellcode; externe Links
  dürfen nur die freigegebenen Adressen öffnen.

**Als Nächstes** — Tempo/Keylock/Scratch (M4), Library mit BPM- und Beatgrid-Analyse (M5), Waveforms (M6), Sync, Hot Cues, Loops und Pads
(M7), Vorhören im Kopfhörer (M8). Siehe [Roadmap](#%EF%B8%8F-roadmap).

## 🎛️ Pioneer DDJ-200

rille erkennt den DDJ-200 beim Anstecken (USB-MIDI; der DDJ-200 hat kein Audio-Interface, der
Ton läuft über das gewählte macOS-Ausgabegerät). Der Controller-Status steht in der Titelleiste,
Vinyl-Modus und MIDI-Monitor in den Einstellungen.

| Element | Funktion | Mit Shift | Status |
|---|---|---|---|
| Play/Pause | Start / Stopp | Rückwärts, solange gehalten | ✅ / M4 |
| Cue | Pioneer-Cue (setzen, vorhören, zurück) | Zum Trackanfang | ✅ |
| Kanalfader | Lautstärke | Fader Start: hoch = Play, zurück auf null = zum Cue | ✅ |
| Crossfader | Überblenden (Kurve in der App) | — | ✅ |
| Jog berühren + Teller | Scratch (Vinyl-Modus) | Schneller Suchlauf | M4 |
| Jog-Rand | Pitch-Bend | Library-Navigation | M4 / M5 |
| Tempo-Fader | Tempo (oben langsamer, wie aufgedruckt) | — | M4 |
| Beat Sync | kurz: Sync · lang: Sync-Lock | Tempo-Bereich wechseln (±6/10/16 %/Wide) | M7 |
| EQ Hi / Mid / Low | 3-Band-Isolator | — | ✅ |
| CFX | Bipolares Filter (links Lowpass, rechts Highpass) | — | ✅ |
| Kopfhörer-Cue | Kanal vorhören | Markierten Track laden | M8 / M5 |
| Master Cue | Master im Kopfhörer | — | M8 |
| Transition FX | Echo-Out / Filter-Out aufs lautere spielende Deck; nochmal = abbrechen | Effekt wechseln | ✅ |
| Pads 1–8 | Hot Cue / Beat Loop / Beat Jump | Löschen / Loop verlassen | M7 |

**Wie der Controller angebunden ist**

- **Das Mapping ist eine Tabelle.** Alle Bytes des Geräts stehen in einer Datei
  ([`ddj200/table.rs`](crates/rille-midi/src/ddj200/table.rs)); ein Test prüft sie gegen jede
  Zeile von Pioneers *DDJ-200 List of MIDI messages*, unabhängig abgetippt.
- **14-Bit-Fader und -Regler** — MSB und LSB werden kombiniert, der Wert gilt beim Eintreffen
  des LSB.
- **Shift wird mitverfolgt** — Jog-Rand und Fader senden mit und ohne Shift dieselbe Nachricht.
- **Soft-Takeover** — nach dem Wiederanstecken oder einer Änderung in der App übernimmt ein
  Regler erst, wenn er den aktuellen Wert erreicht. Mitten im Set springt nichts.
- **LEDs** — Play blinkt bei geladenem, pausiertem Deck, Cue blinkt abseits des Cue-Punkts,
  Loaded leuchtet bei geladenem Track. Gesendet werden nur Änderungen, beim Verbinden alles.
- **Vinyl-Modus** — standardmäßig an, in den Einstellungen umschaltbar, wird beim Verbinden
  gesetzt.

> [!NOTE]
> Das PDF widerspricht sich an einer Stelle: Die Kanaltabelle nennt für Mixer/Effekt MIDI-Kanal
> 5, alle Message-Zeilen nutzen aber Kanal 7 (`B6`/`96`). rille folgt den Message-Zeilen; der
> MIDI-Monitor bestätigt es an echter Hardware. Das PDF ist urheberrechtlich geschützt und liegt
> deshalb nicht im Repository.

**Ohne Hardware:** `cargo run -p rille-midi --example virtual_ddj` startet einen virtuellen
DDJ-200 (CoreMIDI-Virtual-Ports) neben der laufenden App; rille verbindet sich wie mit dem
echten Gerät.

## ⌨️ Maus und Tastatur

Alles funktioniert auch ohne Controller:

| Aktion | So geht's |
|---|---|
| Track laden | Audiodatei aus dem Finder auf ein Deck ziehen |
| Play/Pause, Cue | Klicken, oder Knopf fokussieren und Leertaste / Enter (Cue: halten = vorhören) |
| Zum Anfang | Shift + Klick auf Cue |
| Fader | Ziehen; Pfeiltasten (Shift = große Schritte), Pos1 / Ende; Doppelklick setzt zurück |
| EQ-/Filter-Regler | Hoch/runter ziehen (Shift = fein), Mausrad, Pfeiltasten; Doppelklick oder Entf = Mitte |
| EQ-Kill | Klick auf den Bandnamen (HI / MID / LOW) unter dem Regler |
| Transition-FX | Taste unter dem Crossfader; ⇄ daneben wechselt den Effekt |
| Crossfader-Kurve | Blend / Linear / Cut unter dem Crossfader |
| Einstellungen | ⚙︎ in der Titelleiste oder Klick auf den Audio-/Controller-Status |
| MIDI-Monitor | ⌘⌥M |

Globale Tastenkürzel für die Decks (und eine Übersicht per `?`) kommen mit M9.

## 🚀 Loslegen

**Voraussetzungen:** macOS 13 oder neuer (Apple Silicon empfohlen), Rust stable mit den Xcode
Command Line Tools, Node.js 22 und pnpm 10.

```bash
git clone https://github.com/pepperonas/rille.git
cd rille
pnpm install          # Frontend-Abhängigkeiten inkl. lokaler Tauri-CLI
pnpm tauri dev        # App bauen und starten
```

| Befehl | Was er tut |
|---|---|
| `pnpm tauri dev` | App starten (Vite auf :1420 + Rust, Hot Reload) |
| `pnpm dev` | Nur die Oberfläche im Browser, gegen ein simuliertes Backend (`?demo` für einen lebendigen Zustand) |
| `pnpm check` | Alles, was die CI prüft: Build, Typecheck, ESLint, Vitest, rustfmt, Clippy, cargo test |
| `pnpm e2e` | Playwright-Tests gegen das simulierte Backend (nutzt das installierte Chrome) |
| `pnpm badges` | Die Zahlen im README neu berechnen (`.github/badges/*.json`) |
| `pnpm screenshots` | `docs/screenshots/*` und `docs/banner.png` neu rendern |
| `pnpm tokens` | Farb-Tokens nach Änderung einer Seed-Farbe neu erzeugen |
| `pnpm tauri build` | `rille.app` / `.dmg` bauen (vorerst unsigniert) |

## 🔊 Audio-Engine

- **Echtzeit-Regeln.** Der CoreAudio-Callback alloziert nie, sperrt nie, loggt nicht und macht
  kein I/O. Ein Test rendert 2.000 Blöcke unter
  [`assert_no_alloc`](https://crates.io/crates/assert_no_alloc) — und eine Gegenprobe beweist,
  dass der Wächter eine Allokation bemerken würde. Debug-Builds der App brechen laut ab, falls
  der Callback je alloziert.
- **Lock-frei rein und raus.** Oberfläche und Controller haben je eine eigene
  [`rtrb`](https://crates.io/crates/rtrb)-Befehlsqueue; die Engine veröffentlicht ihren Zustand
  über einen Triple Buffer (der neueste gewinnt, kein Rückstau). Ersetzte Tracks wandern über
  eine Rückgabe-Queue zurück, im Audio-Thread wird nie Speicher freigegeben.
- **Ohne Gerät testbar.** `Engine::process(&mut out, channels)` kennt cpal nicht; Tests rendern
  offline und messen.
- **Überall geglättet.** Fader und Crossfader werden pro Sample (zeitbasiert) geglättet, nichts
  „zippert“. Kennlinien: Kanalfader quadratisch (−12 dB auf halber Höhe), Crossfader konstante
  Leistung / linear / Cut.
- **Declick.** Stopps, Starts, Cue-Sprünge und Seeks werden zu 4-ms-Blenden.
- **Gerätewechsel.** Ein wartefreier `EngineSlot` reicht die Engine von einem Ausgabe-Stream zum
  nächsten; geladene Tracks, Positionen und Cue-Punkte überstehen den Wechsel. cpal meldet
  Fehler teils direkt im Render-Thread und jede CoreAudio-Überlastung als „Xrun“-Fehler — rille
  fasst dort nur Atomics an, zählt Überlastungen und baut den Stream nur neu auf, wenn er
  wirklich kaputt ist (aufs Standardgerät, wenn das alte verschwunden ist).
- **Dekodieren** mit [symphonia](https://github.com/pdeljanov/Symphonia), Resampling mit
  [rubato](https://github.com/HEnquist/rubato) (FFT), begrenzt auf 1 h / 200 Mio. Frames und
  8–384 kHz, damit ein kaputter Header keinen Speicher auffressen kann.

## 🏛️ Architektur

```
 React-UI ──Befehle──▶ src-tauri ──rtrb──▶ ┌──────────────────────────┐
    ▲                    │                 │ rille-engine             │ CoreAudio
    │ 60-Hz-Zustand      │ AudioService    │ Engine::process (RT)     ├──────────▶ Ausgabe
    └── Tauri-Channel ◀──┤ Bridge-Thread ◀─┤ Triple Buffer · Events   │
                         │                 └──────────────────────────┘
 DDJ-200 ◀── LEDs ──── ControllerService ──rtrb──▶ (eigene Befehlsqueue)
   USB-MIDI ──────────▶ rille-midi: Tabelle → Decoder → Soft-Takeover → Belegung
                         rille-library: Dekodieren (symphonia) · Resampling (rubato)
```

| Thread | Aufgabe | Darf |
|---|---|---|
| Main (AppKit/Tauri) | Fenster, Commands, erster CoreMIDI-Client | alles außer blockieren |
| Audio (CoreAudio) | `Engine::process`, Queues, Snapshot | nur lock- und allokationsfrei |
| Bridge (60 Hz) | Zustand an die Oberfläche, Freigaben, Stream-Zustand | normal |
| Controller | MIDI rein → Engine-Befehle, Engine-Zustand → LEDs, Hotplug | normal |
| Loader | Dekodieren + Resampling, Tracks an die Engine | normal |

- **IPC:** typisierte Tauri-Commands für Aktionen; ein `Channel` streamt den Zustand mit bis zu
  60 Hz und nur, solange sich etwas ändert oder spielt (im Leerlauf 0 Hz). Die Oberfläche rechnet
  Positionen zwischen zwei Frames fort, Zeitanzeigen laufen mit Bildschirmrate (120 Hz auf
  ProMotion).
- **Sicherheit:** strikte CSP (keine Remote-Inhalte, Schriften gebündelt), minimale
  Tauri-Capabilities.
- Das Frontend rechnet nichts Audio-Relevantes; ein Mock-Backend mit derselben Schnittstelle
  lässt die Oberfläche für Entwicklung und Tests im normalen Browser laufen.

## 🎨 Design

- **Material 3 Expressive**, dunkel zuerst. Die Farben entstehen aus einer Seed-Farbe plus einem
  Akzent pro Deck (Cyan / Orange) mit
  [material-color-utilities](https://github.com/material-foundation/material-color-utilities);
  ein Test prüft Textkontrast (≥ 4,5:1) und Sichtbarkeit der Akzente (≥ 3:1) in beiden Themes.
- **Tokens sind die einzige Quelle** (`src/design/tokens.css`); ein Test schlägt fehl, wenn ein
  Komponenten-Stylesheet einen rohen Pixelwert oder eine Farbe enthält.
- **Bewegung** nur mit Springs; Effekt-Springs schwingen nie über (getestet), und
  `prefers-reduced-motion` schaltet Animationen ab.
- **Roboto Flex** als Variable Font, lokal gebündelt; breite Schnitte für Zeiten und Titel.
- **Privat by Design:** kein Netzwerkzugriff, kein Tracking, kein Konto.

## 📏 Performance

| Budget | Ziel | Bisher gemessen |
|---|---|---|
| Puffer | 256 Frames @ 48 kHz ohne Aussetzer | 256 Frames fest, 0 Aussetzer (MacBook, Leerlauf) |
| Ausgabelatenz | — | 5,3 ms (Puffer + Gerät) |
| CPU im Leerlauf | minimal | 0,2 % (Debug-Build, nichts spielt) |
| 3:43-MP3 dekodieren + resampeln | Track laden < 300 ms (analysierte Tracks) | ~360 ms (Release) — ein PCM-Cache kommt mit der Library (M5) |
| Controller → hörbar | < 10 ms (ohne Gerätelatenz) | ein Audioblock (≤ 5,3 ms) + MIDI |

## 🧪 Tests

Die Badges oben sind gemessen, nicht getippt: `tools/stats.mjs` zählt Tests und Zeilen, und die
CI committet neue Zahlen, sobald sie sich ändern.

| Suite | Wo | Schwerpunkte |
|---|---|---|
| Rust Unit + Integration | `crates/*/src`, `crates/*/tests`, `src-tauri/src` | Cue-Zustandsmaschine, Kennlinien, Glättung, Declick, Pegel, gemessene EQ-/Filter-Frequenzgänge, Decoder-Randfälle, jede PDF-Zeile des DDJ-200, 14 Bit/Jog/Shift, Soft-Takeover inkl. Reconnect und schneller Fader-Züge |
| Allokations-Wächter | `crates/rille-engine/tests/no_alloc.rs` | 2.000 Blöcke unter `assert_no_alloc`, plus Gegenprobe mit absichtlicher Allokation |
| CoreMIDI-Rundweg | `crates/rille-midi/tests/virtual_roundtrip.rs` | virtueller DDJ-200 → Port → Decoder → Aktion, und LEDs zurück |
| Frontend (Vitest) | `src/**/*.test.ts`, `scripts`, `tools` | Store und Lade-Rennen, Positions-Fortschreibung, Token-Gleichstand und Kontrast, „keine Rohwerte“-Wächter |
| E2E (Playwright) | `e2e/` | Laden, Play, Cue, Auswerfen, Tastaturbedienung, Fader, Einstellungen, Controller-UI — gegen das Mock-Backend |

Praxis: Clippy mit `-D warnings`, `unwrap`/`expect`/`panic` außerhalb von Tests verboten, neue
Wächter werden per Mutationsprobe geprüft (einmal gegen absichtlich kaputten Code laufen lassen,
um zu sehen, dass sie anschlagen).

```bash
pnpm check   # was die CI prüft
pnpm e2e     # Browser-Tests
```

## 🗂️ Projektstruktur

```
rille/
├── crates/
│   ├── rille-core/      gemeinsame Typen: DeckId, Command, Snapshot, TrackAudio
│   ├── rille-engine/    Echtzeit-Engine, DSP, cpal-Ausgabe, EngineSlot
│   ├── rille-midi/      DDJ-200-Tabelle, Decoder, Belegung, Soft-Takeover, LEDs, Ports, virtueller DDJ
│   └── rille-library/   Dekodieren und Resampling (Library + Analyse folgen in M5)
├── src-tauri/           App-Shell: AudioService, ControllerService, Commands, Bridge
├── src/                 React-Oberfläche: deck/, mixer/, settings/, dev/, design/, ipc/, state/
├── e2e/                 Playwright-Tests
├── tools/               Statistik (Badges), Screenshots, Banner und Fensterrahmen
├── scripts/             Generator für die Farb-Tokens
├── docs/                Banner, Screenshots, Pläne (docs/superpowers/plans)
└── .github/             CI und Badge-Daten
```

## 🗺️ Roadmap

| Meilenstein | Inhalt | Stand |
|---|---|---|
| M0 | Fundament: Workspace, Tauri-Shell, Tokens, CI | ✅ |
| M1 | Engine-Kern: Wiedergabe, Cue, Mixer, Pegel, Geräte | ✅ |
| M2 | Pioneer DDJ-200: Mapping, LEDs, Takeover, Hotplug, Monitor | ✅ (Hardware-Abnahme offen) |
| M3 | Isolator-EQ mit Kill, bipolares Filter, Transition-FX (Echo-Out, Filter-Out) | ✅ |
| M4 | Tempo-Bereiche, Keylock (Signalsmith Stretch), Pitch-Bend, Scratch, Reverse | ⏳ |
| M5 | Library: Import, SQLite, BPM-/Beatgrid-Analyse, Suche, Drag & Drop | ⏳ |
| M6 | Waveforms (Übersicht + scrollende Detailansicht, Frequenzfarben), Deck-Oberfläche | ⏳ |
| M7 | Sync & Sync-Lock, Hot Cues, Beat Loops, Beat Jump, Pad-Modi | ⏳ |
| M8 | Vorhören über ein zweites Gerät oder ein 4-Kanal-Interface | ⏳ |
| M9 | Vollständige Tastatursteuerung, helles Theme, Leerzustände, gemessene Budgets | ⏳ |

Später: Tonart-Erkennung, weitere Effekte, Aufnahme, Sampler-Pads.

## 🧯 Fehlerbehebung

**Kein Ton / „Kein Audio-Gerät“.** In den Einstellungen ein Ausgabegerät wählen. rille läuft mit
48 kHz (44,1 kHz, falls das Standardgerät das nicht kann); ein Gerät, das beides nicht kann,
meldet einen klaren Fehler.

**Der DDJ-200 wird nicht erkannt.** Ein datenfähiges USB-Kabel nutzen und prüfen, ob der
Controller in *Audio-MIDI-Setup → MIDI-Studio* auftaucht. rille sucht einmal pro Sekunde; der
MIDI-Monitor (⌘⌥M) zeigt, ob Nachrichten ankommen.

**Aussetzer.** Der Aussetzer-Zähler steht in den Einstellungen. Puffer erhöhen (512 oder 1024)
und Apps schließen, die dasselbe Audiogerät nutzen.

**Ein Fader am Controller tut nichts.** Soft-Takeover: einmal über den Wert ziehen, den die App
zeigt, dann übernimmt er.

## 🤝 Mitmachen

Issues und Pull Requests sind willkommen. Bitte vorher [`CLAUDE.md`](CLAUDE.md) und die Regeln in
[`.claude/rules/`](.claude/rules) lesen (Echtzeit-Audio, MIDI-Mapping, Design) und vor einem PR
`pnpm check` und `pnpm e2e` laufen lassen. Commits folgen
[Conventional Commits](https://www.conventionalcommits.org). Code aus GPL-Projekten (etwa die
Mixxx-Mappings) darf nicht in rille übernommen werden.

Änderungen stehen im [Changelog](CHANGELOG.md).

## ❤️ Unterstützen

rille ist kostenlos, quelloffen und entsteht abends von einer Person. Wenn es deine Sets besser
macht, helfen ein Kaffee oder eine Bewertung sehr:

<a href="https://www.paypal.com/donate/?business=martin.pfeffer@celox.io&currency_code=EUR&item_name=rille"><img src="https://img.shields.io/badge/Spenden-PayPal-00457C?logo=paypal&logoColor=white&style=for-the-badge" alt="Spenden via PayPal"></a>
<a href="https://g.page/r/CXgdRV3QysvxEBM/review"><img src="https://img.shields.io/badge/celox.io%20bewerten-Google%20Maps%20%E2%AD%90-4285F4?logo=googlemaps&logoColor=white&style=for-the-badge" alt="celox.io auf Google Maps bewerten"></a>

- ☕ **Spenden** über PayPal: [martin.pfeffer@celox.io](https://www.paypal.com/donate/?business=martin.pfeffer@celox.io&currency_code=EUR&item_name=rille)
- ⭐ **celox.io bewerten** auf Google Maps: [g.page/r/CXgdRV3QysvxEBM/review](https://g.page/r/CXgdRV3QysvxEBM/review)
- 🌟 Repo mit einem Stern versehen, Issues melden, PRs schicken — genauso willkommen.

## 📄 Lizenz

[MIT](LICENSE) © 2026 Martin Pfeffer · [celox.io](https://celox.io)

**Fremdkomponenten:** Roboto Flex (SIL Open Font License 1.1), material-color-utilities
(Apache 2.0, nur zur Build-Zeit) sowie die Rust- und npm-Abhängigkeiten aus `Cargo.lock` und
`pnpm-lock.yaml` unter ihren eigenen Lizenzen. Pioneer DJ und DDJ-200 sind Marken ihrer
jeweiligen Inhaber; rille steht in keiner Verbindung zu ihnen und wird nicht von ihnen
unterstützt.
