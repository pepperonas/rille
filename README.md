<div align="center">

<a href="https://github.com/pepperonas/rille"><img src="docs/banner.png" alt="rille — native DJ app for macOS, made for the Pioneer DDJ-200" width="100%"></a>

# 🎛️ rille

**English** · [Deutsch](README.de.md)

**A native DJ app for macOS with a realtime Rust audio engine, built around the Pioneer DDJ-200 — and fully playable with mouse and keyboard.**

<p>
  <a href="https://www.paypal.com/donate/?business=martin.pfeffer@celox.io&currency_code=EUR&item_name=rille"><img alt="Donate with PayPal" height="46" src="https://img.shields.io/badge/%E2%98%95_Donate-via_PayPal-00457C?style=for-the-badge&logo=paypal&logoColor=white"></a>
  &nbsp;
  <a href="https://g.page/r/CXgdRV3QysvxEBM/review"><img alt="Rate celox.io on Google Maps" height="46" src="https://img.shields.io/badge/%E2%AD%90_Rate_celox.io-on_Google_Maps-4285F4?style=for-the-badge&logo=googlemaps&logoColor=white"></a>
</p>

<!-- BADGES:BIG — computed by tools/stats.mjs, refreshed by CI on every push to main. -->
[![version](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/pepperonas/rille/main/.github/badges/version.json&style=for-the-badge)](CHANGELOG.md)
[![unit tests](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/pepperonas/rille/main/.github/badges/unit-tests.json&style=for-the-badge)](#-testing)
[![lines of code](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/pepperonas/rille/main/.github/badges/loc.json&style=for-the-badge)](#-project-structure)
[![test code](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/pepperonas/rille/main/.github/badges/test-code.json&style=for-the-badge)](#-testing)

</div>

<!-- Live numbers (same source, small) -->

[![Rust tests](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/pepperonas/rille/main/.github/badges/tests-rust.json)](crates)
[![frontend tests](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/pepperonas/rille/main/.github/badges/tests-frontend.json)](src)
[![e2e tests](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/pepperonas/rille/main/.github/badges/tests-e2e.json)](e2e)
[![Rust LoC](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/pepperonas/rille/main/.github/badges/loc-rust.json)](crates)
[![TypeScript LoC](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/pepperonas/rille/main/.github/badges/loc-ts.json)](src)
[![CSS LoC](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/pepperonas/rille/main/.github/badges/loc-css.json)](src)

<!-- Project status — GitHub badges update themselves. -->

[![CI](https://img.shields.io/github/actions/workflow/status/pepperonas/rille/ci.yml?branch=main&label=CI&logo=githubactions&logoColor=white)](https://github.com/pepperonas/rille/actions/workflows/ci.yml)
[![Latest tag](https://img.shields.io/github/v/tag/pepperonas/rille?sort=semver&label=tag&logo=github&logoColor=white&color=5B4BFF)](https://github.com/pepperonas/rille/tags)
[![Last commit](https://img.shields.io/github/last-commit/pepperonas/rille?logo=git&logoColor=white)](https://github.com/pepperonas/rille/commits/main)
[![Commit activity](https://img.shields.io/github/commit-activity/m/pepperonas/rille?logo=git&logoColor=white)](https://github.com/pepperonas/rille/commits/main)
[![Code size](https://img.shields.io/github/languages/code-size/pepperonas/rille?logo=files&logoColor=white)](#-project-structure)
[![Repo size](https://img.shields.io/github/repo-size/pepperonas/rille?logo=github&logoColor=white)](https://github.com/pepperonas/rille)
[![Top language](https://img.shields.io/github/languages/top/pepperonas/rille?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![Open issues](https://img.shields.io/github/issues/pepperonas/rille?logo=github&logoColor=white)](https://github.com/pepperonas/rille/issues)
[![Stars](https://img.shields.io/github/stars/pepperonas/rille?logo=github&logoColor=white)](https://github.com/pepperonas/rille/stargazers)
[![License: MIT](https://img.shields.io/github/license/pepperonas/rille?color=blue)](LICENSE)
[![Made by celox.io](https://img.shields.io/badge/made%20by-celox.io-5B4BFF)](https://celox.io)

<!-- Platform & runtime -->

[![Platform](https://img.shields.io/badge/platform-macOS%2013%2B-000000?logo=apple&logoColor=white)](#-getting-started)
[![Apple Silicon](https://img.shields.io/badge/Apple%20Silicon-native-000000?logo=apple&logoColor=white)](#-getting-started)
[![CoreAudio](https://img.shields.io/badge/audio-CoreAudio%20via%20cpal-000000?logo=apple&logoColor=white)](#-audio-engine)
[![CoreMIDI](https://img.shields.io/badge/MIDI-CoreMIDI%20via%20midir-000000?logo=apple&logoColor=white)](#%EF%B8%8F-pioneer-ddj-200)
[![Buffer](https://img.shields.io/badge/buffer-256%20frames%20%40%2048%20kHz-2E9E5B)](#-performance)
[![Formats](https://img.shields.io/badge/formats-MP3%20%C2%B7%20AAC%20%C2%B7%20ALAC%20%C2%B7%20FLAC%20%C2%B7%20WAV%20%C2%B7%20AIFF-4B6BDF)](#-features)
[![Controller](https://img.shields.io/badge/controller-Pioneer%20DDJ--200-E53935)](#%EF%B8%8F-pioneer-ddj-200)

<!-- Language, build & toolchain -->

[![Rust](https://img.shields.io/badge/Rust-2024%20edition%20%C2%B7%201.98-DEA584?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![Tauri](https://img.shields.io/badge/Tauri-2.12-24C8DB?logo=tauri&logoColor=white)](https://tauri.app)
[![React](https://img.shields.io/badge/React-19-61DAFB?logo=react&logoColor=black)](https://react.dev)
[![TypeScript](https://img.shields.io/badge/TypeScript-strict-3178C6?logo=typescript&logoColor=white)](https://www.typescriptlang.org)
[![Vite](https://img.shields.io/badge/Vite-8-646CFF?logo=vite&logoColor=white)](https://vite.dev)
[![pnpm](https://img.shields.io/badge/pnpm-10-F69220?logo=pnpm&logoColor=white)](https://pnpm.io)
[![cpal](https://img.shields.io/badge/cpal-0.18-DEA584?logo=rust&logoColor=white)](https://github.com/RustAudio/cpal)
[![symphonia](https://img.shields.io/badge/symphonia-0.6-DEA584?logo=rust&logoColor=white)](https://github.com/pdeljanov/Symphonia)
[![rubato](https://img.shields.io/badge/rubato-5-DEA584?logo=rust&logoColor=white)](https://github.com/HEnquist/rubato)
[![rtrb](https://img.shields.io/badge/rtrb-lock--free%20queues-DEA584?logo=rust&logoColor=white)](https://github.com/mgeier/rtrb)
[![midir](https://img.shields.io/badge/midir-0.11-DEA584?logo=rust&logoColor=white)](https://github.com/Boddlnagg/midir)

<!-- Design -->

[![Material 3 Expressive](https://img.shields.io/badge/Material%203-Expressive-6750A4?logo=materialdesign&logoColor=white)](https://m3.material.io)
[![Dark first](https://img.shields.io/badge/theme-dark%20first-1C1B1F)](#-design)
[![Motion](https://img.shields.io/badge/motion-springs%20%C2%B7%20reduced--motion%20aware-6750A4)](#-design)
[![Type](https://img.shields.io/badge/type-Roboto%20Flex%20%C2%B7%20OFL-4285F4?logo=googlefonts&logoColor=white)](https://fonts.google.com/specimen/Roboto+Flex)
[![Contrast](https://img.shields.io/badge/contrast-WCAG%20AA%20tested-2E9E5B)](#-design)

<!-- Engineering practice -->

[![Realtime safe](https://img.shields.io/badge/audio%20callback-allocation--free%20(tested)-2E9E5B?logo=rust&logoColor=white)](#-audio-engine)
[![Clippy](https://img.shields.io/badge/clippy-%E2%80%91D%20warnings-DEA584?logo=rust&logoColor=white)](.github/workflows/ci.yml)
[![No unwrap](https://img.shields.io/badge/unwrap%2Fexpect%2Fpanic-denied%20outside%20tests-DEA584?logo=rust&logoColor=white)](Cargo.toml)
[![Playwright](https://img.shields.io/badge/e2e-Playwright-2EAD33?logo=playwright&logoColor=white)](e2e)
[![Vitest](https://img.shields.io/badge/unit-Vitest-6E9F18?logo=vitest&logoColor=white)](src)
[![Conventional Commits](https://img.shields.io/badge/commits-Conventional-FE5196?logo=conventionalcommits&logoColor=white)](https://www.conventionalcommits.org)
[![SemVer](https://img.shields.io/badge/SemVer-2.0.0-303030?logo=semver&logoColor=white)](https://semver.org)
[![Keep a Changelog](https://img.shields.io/badge/changelog-Keep%20a%20Changelog-E05735?logo=keepachangelog&logoColor=white)](CHANGELOG.md)
[![PRs welcome](https://img.shields.io/badge/PRs-welcome-brightgreen)](#-contributing)

<!-- What rille deliberately does not do -->

[![Offline](https://img.shields.io/badge/network-none-2E7D32)](#-design)
[![No trackers](https://img.shields.io/badge/trackers-none-2E7D32)](#-design)
[![No account](https://img.shields.io/badge/account-not%20required-2E7D32)](#-design)
[![No CDN](https://img.shields.io/badge/remote%20content-none%20(strict%20CSP)-2E7D32)](#-architecture)

> **Status: early development (v0.1.0).** Two decks play files dropped from the Finder, with
> Pioneer-style cue, tempo with keylock, pitch bend, scratch and reverse, isolator EQ, filter,
> transition FX, channel faders, crossfader and meters — controllable with mouse, keyboard and
> the DDJ-200, plus a library with BPM/beatgrid analysis and waveforms. Beat features follow milestone by milestone (see [Roadmap](#%EF%B8%8F-roadmap)). There is no packaged release yet.

---

## Table of contents

- [📸 Screenshots](#-screenshots)
- [✨ Features](#-features)
- [🎛️ Pioneer DDJ-200](#%EF%B8%8F-pioneer-ddj-200)
- [⌨️ Mouse and keyboard](#%EF%B8%8F-mouse-and-keyboard)
- [🚀 Getting started](#-getting-started)
- [🔊 Audio engine](#-audio-engine)
- [🏛️ Architecture](#%EF%B8%8F-architecture)
- [🎨 Design](#-design)
- [📏 Performance](#-performance)
- [🧪 Testing](#-testing)
- [🗂️ Project structure](#%EF%B8%8F-project-structure)
- [🗺️ Roadmap](#%EF%B8%8F-roadmap)
- [🧯 Troubleshooting](#-troubleshooting)
- [🤝 Contributing](#-contributing)
- [❤️ Support](#%EF%B8%8F-support)
- [📄 License](#-license)

---

## 📸 Screenshots

<p align="center">
  <img src="docs/screenshots/decks.png" alt="Two decks playing: track title, elapsed and remaining time, transport, mixer with channel faders and meters" width="100%">
</p>

| First start | Settings | MIDI monitor |
|:---:|:---:|:---:|
| <img src="docs/screenshots/empty.png" alt="Empty decks and library explain what to do" width="100%"> | <img src="docs/screenshots/settings.png" alt="Settings: output device, buffer size, latency, dropouts, controller, vinyl mode" width="100%"> | <img src="docs/screenshots/midi-monitor.png" alt="MIDI monitor with decoded controller messages" width="100%"> |
| Empty states say what to do next | Device, buffer, latency, dropouts, controller | Every MIDI message, decoded, live (⌘⌥M) |

| Light theme | Keyboard shortcuts |
|:---:|:---:|
| <img src="docs/screenshots/light.png" alt="The same two decks in the light theme" width="100%"> | <img src="docs/screenshots/shortcuts.png" alt="Shortcut overview opened with ?" width="100%"> |
| Dark, light or follow the system (settings) | `?` lists every key; the same table drives the handler |

<sub>Screenshots are rendered from the in-browser demo (`pnpm screenshots`), so they are
reproducible without hardware. The MIDI traffic in the monitor shot is simulated.</sub>

## ✨ Features

**Available now**

- **Two decks** — drop an audio file from the Finder onto a deck, or load from the library;
  title, artist, BPM, elapsed and remaining time (tabular digits, readable from two metres),
  progress line, eject.
- **Instant loading** — a track is playable about 4 ms after loading starts and fills while it
  plays (progressive decode into a lock-free buffer), measured on a 4:18 MP3 that takes 450 ms
  to decode and resample completely.
- **Library** — import a folder (button or drop it from the Finder onto the library); title,
  artist and album come from the tags, or from an `Artist - Title` file name. A virtualised list
  stays fast with tens of thousands of tracks; search across title, artist, album and BPM
  (accents ignored), sort by title, artist, BPM or duration. Load by double-click, Enter (first
  empty deck), `1` / `2`, by dragging a row onto a deck, or from the DDJ-200 (Shift + jog rim
  browses, Shift + headphone cue loads). Stored in SQLite; changed files are re-analysed.
- **BPM and beatgrid analysis** — in the background on all cores but two, at a low system
  priority: spectral-flux onsets, autocorrelation tempo with an octave check, refinement to
  0.005 BPM by phase folding, and a downbeat-phase estimate from the kick band. A track without
  a clear, steady beat gets *no* tempo instead of a guessed one. On 21 real tracks (house,
  techno, disco, pop, a live drummer) every one got a tempo; an 8-minute track takes ~1.5 s.
- **Waveforms** — a scrolling detail view (WebGL2, one draw call per frame) with the beatgrid
  (stronger line every fourth beat) and four zoom steps from 2 to 32 s (mouse wheel or + / −,
  shared by both decks), and an overview of the whole track with played part, cue mark and
  click-to-seek. Three frequency bands in shades of the deck colour. They draw at display rate
  only while a deck plays; standing still costs nothing. A file dropped onto a deck joins the
  library and its analysis jumps the queue.
- **Remaining-time warning** — the last 30 seconds turn the remaining time red and let it pulse
  (steady with reduced motion).
- **Sync and sync lock** — SYNC matches tempo and phase to the other deck once (at half or
  double tempo when that is closer: 174 follows 87, widening the tempo range if needed); LOCK
  keeps following it and pulls the phase in with at most 2 % speed change, never with a jump.
- **Hot cues, loops, beat jump** — eight hot cues per deck (snapped to a beat within ⅛ beat,
  saved in the library), beat loops from ¼ to 32 beats that repeat exactly at any tempo (the
  audio block is split at the loop end), beat jumps of ½, 1, 4 and 16 beats that take a
  running loop along. Shown in both waveforms.
- **Pads** — eight pads per deck in three modes (Hot Cue, Loop, Jump), chosen in the app; the
  DDJ-200 pads follow it, their LEDs show set cues and blink the active loop with the beat.
- **Beatgrid correction** — move the grid by 10 ms, halve or double the tempo, fine-tune by
  0.01 BPM, or tap it in; saved in the library and applied to a playing deck at once.
- **Pioneer-style cue** — paused: set the cue point; hold on the cue point to preview and
  release to return; press Play while holding to keep playing; while playing: back to cue and
  pause. Shift + Cue jumps to the start.
- **Tempo** — tempo fader per deck (top = slower, as on the controller) with four ranges:
  ±6 %, ±10 %, ±16 % and wide (±50 %); the change is shown in percent. Without keylock the
  pitch follows the tempo, like a turntable (4-point Hermite interpolation).
- **Keylock** — keeps the pitch while the tempo changes, with
  [Signalsmith Stretch](https://signalsmith-audio.co.uk/code/stretch/). Its latency is
  compensated, so the stretched audio lines up with the playhead (measured: within 2 ms), and
  after a cue jump a short varispeed bridge covers the stretcher's start-up — no gap. Loops
  run seamlessly with keylock: the stretcher is fed the loop as one continuous stream (each
  seam crossfaded over 4 ms), so every pass keeps the pitch from its first frame.
- **Pitch bend, scratch, search, reverse** — hold − / + to nudge (±4 %), the jog rim bends,
  the platter scratches in vinyl mode, Shift + platter searches, REV plays backwards while held
  and stops at the track start.
- **Mixer** — channel faders with a DJ taper, master level, crossfader with three curves
  (Blend = constant power, Linear, Cut for scratching), segmented peak meters per channel and
  master.
- **Isolator EQ** — Hi / Mid / Low per channel with Linkwitz-Riley crossovers at 200 Hz and
  2 kHz (flat when neutral), knob fully left or a click on the band name kills the band, up
  to +6 dB boost.
- **Bipolar filter** — one knob per channel: left a low-pass closes, right a high-pass opens,
  the centre is a true bypass.
- **Transition FX** — one button takes the louder playing deck out of the mix: *Echo-Out*
  (the deck fades while its last moment keeps echoing) or *Filter-Out* (a high-pass sweeps
  up). The deck pauses afterwards; pressing again cancels.
- **Click-free transport** — stop, start, cue jumps and seeks crossfade over 4 ms instead of
  cutting the waveform.
- **Audio settings** — output device, buffer size 64–2048, measured latency, dropout counter;
  unplugging the interface switches to the system default without stopping the set.
- **Formats** — MP3, AAC/M4A, ALAC, FLAC, WAV, AIFF; resampled to the engine rate on load.
- **Pioneer DDJ-200** — plug and play, LED feedback, soft takeover, vinyl mode, MIDI monitor
  (details [below](#%EF%B8%8F-pioneer-ddj-200)).
- **Material 3 Expressive UI** — dark first, per-deck accent colours, spring motion, the play
  button morphs between circle and rounded square.
- **About dialog** — version, donate and rate buttons, links to the source; external links may
  open only the allow-listed URLs.

- **Headphones** — pre-listen any channel before its fader (cue per channel, master cue,
  cue/master blend, level). The DDJ-200 has no sound card, so the headphones go to outputs 3/4
  of a four-output interface or to a second device (USB headphones, the Mac's headphone jack
  next to an external interface). A second device runs on its own clock; rille follows it
  (±200 ppm without a dropout, the speed correction stays below 0.06 %), and if it is
  unplugged the master plays on.

- **Keyboard first** — every deck and mixer function has a key (same physical keys on QWERTZ
  and QWERTY); `?` shows them all, generated from the table the handler uses. Held keys (cue,
  bend) let go when the window loses focus.
- **Light and dark** — dark by default, light, or follow macOS; the native window follows too.
  The start tempo range and the crossfader curve are remembered.
- **Empty states** — no output device, no controller, no tracks: each says what to do next.

**Coming next** — hardware acceptance with the DDJ-200. See the [Roadmap](#%EF%B8%8F-roadmap).

## 🎛️ Pioneer DDJ-200

rille detects the DDJ-200 when it is plugged in (USB-MIDI; the DDJ-200 has no audio interface,
so sound goes to the selected macOS output). The controller status is in the title bar; vinyl
mode and the MIDI monitor are in the settings.

| Control | Function | With Shift | Status |
|---|---|---|---|
| Play/Pause | Start / stop | Reverse while held | ✅ |
| Cue | Pioneer cue (set, preview, back) | Jump to track start | ✅ |
| Channel fader | Level | Fader start: up = play, back to zero = return to cue | ✅ |
| Crossfader | Blend (curve chosen in the app) | — | ✅ |
| Jog touch + platter | Scratch (vinyl mode); bend with vinyl mode off | Fast search | ✅ |
| Jog rim | Pitch bend | Browse the library | ✅ |
| Tempo fader | Tempo (top = slower, as printed), soft takeover | — | ✅ |
| Beat Sync | Short: sync · long: sync lock (LED = lock) | Cycle tempo range (±6/10/16 %/wide) | ✅ |
| EQ Hi / Mid / Low | Three-band isolator EQ | — | ✅ |
| CFX | Bipolar filter (left low-pass, right high-pass) | — | ✅ |
| Headphone cue | Pre-listen the channel (LED = on) | Load the selected track | ✅ |
| Master cue | Master in the headphones (LED = on) | — | ✅ |
| Transition FX | Echo-out / filter-out on the louder playing deck; again = cancel | Change effect | ✅ |
| Pads 1–8 | Hot cue / beat loop (¼–32) / beat jump (top row back ½·1·4·16, bottom forward), mode chosen in the app | Delete cue / exit loop | ✅ |

**How the controller is handled**

- **Mapping is a table.** All bytes of the device live in one file
  ([`ddj200/table.rs`](crates/rille-midi/src/ddj200/table.rs)), checked by a test against every
  row of Pioneer's *DDJ-200 List of MIDI messages* typed in independently.
- **14-bit faders and knobs** — MSB and LSB are combined, the value is taken when the LSB
  arrives.
- **Shift is tracked** — the jog rim and the faders send the same message with and without
  Shift.
- **Soft takeover** — after reconnecting, or after a value was changed in the app, a hardware
  control only takes over once it reaches the current value. Nothing jumps mid-set.
- **LEDs** — Play blinks on a loaded, paused deck, Cue blinks away from the cue point, Loaded
  lights up when a track is on the deck. Only changes are sent; everything is resent on
  connect.
- **Vinyl mode** — on by default, switchable in the settings; sent to the controller on connect.

> [!NOTE]
> The PDF is ambiguous in one place: its channel table lists the mixer/effect section on MIDI
> channel 5, while every message row uses channel 7 (`B6`/`96`). rille follows the message rows;
> the MIDI monitor confirms it on real hardware. The PDF itself is copyrighted by Pioneer DJ and
> therefore not part of this repository.

**Without hardware:** `cargo run -p rille-midi --example virtual_ddj` starts a virtual DDJ-200
(CoreMIDI virtual ports) next to the running app; rille connects to it like to the real device.

## ⌨️ Mouse and keyboard

Everything works without a controller:

| Action | How |
|---|---|
| Load a track | Drop an audio file from the Finder onto a deck, or a library row (drag, double-click) |
| Seek | Click into the overview waveform under the time |
| Waveform zoom | Mouse wheel over the detail waveform, or focus it and press + / − |
| Sync | SYNC (once) and LOCK (keep following) next to Play |
| Pads | Choose Hot Cue / Loop / Jump above the pads; click a pad; Shift + click deletes a cue or leaves the loop |
| Headphones | 🎧 next to 1 / 2 / M in the mixer; MIX (cue ↔ master) and PHONES (level) above the master fader; output in the settings |
| Beatgrid | GRID next to the BPM: ◀ ▶ move it, ÷2 ×2, ±0.01 BPM, TAP in time |
| Import music | "Ordner importieren", or drop a folder onto the library |
| Library | Search field (↓ / Enter jumps into the list); in the list ↑ ↓, Page ↑ ↓, Home / End; Enter = first empty deck, `1` / `2` = deck 1 / 2; click a column to sort |
| Play / Pause, Cue | Click, or focus the button and press Space / Enter (Cue: hold to preview) |
| Jump to start | Shift + click on Cue |
| Tempo | Tempo fader right of the deck: drag (top = slower), arrow keys, double-click = 0 % |
| Tempo range | Chip above the tempo fader (±6 % → ±10 % → ±16 % → wide) |
| Keylock | KEY button |
| Pitch bend, reverse | Hold − / + or REV (mouse or Space / Enter) |
| Faders | Drag; arrow keys (Shift = larger steps), Home / End; double-click resets |
| EQ / filter knobs | Drag up/down (Shift = fine), mouse wheel, arrow keys; double-click or Delete = centre |
| EQ kill | Click the band name (HI / MID / LOW) under the knob |
| Transition FX | Button under the crossfader; ⇄ next to it changes the effect |
| Crossfader curve | Blend / Linear / Cut chips under the crossfader |
| Settings | ⚙︎ in the title bar, or click the audio / controller status |
| MIDI monitor | ⌘⌥M |

### Keyboard shortcuts

Keys sit at the same place on QWERTZ and QWERTY (Y and Z are not used). `?` shows the same list
in the app. Shortcuts rest while a text field or a dialog has the focus.

| Deck 1 | Deck 2 | Action |
|---|---|---|
| `F` | `J` | Play / Pause |
| `D` | `K` | Cue (hold = preview) |
| `S / ⇧S` | `L / ⇧L` | Sync · with ⇧ sync lock on/off |
| `G` | `H` | Headphone cue on/off |
| `1 2 3 4 Q W E R` | `7 8 9 0 U I O P` | Pads 1–8 in the chosen mode · with ⇧ delete / leave the loop |
| `5` | `6` | Next pad mode (Hot Cue → Loop → Jump) |
| `X / C` | `M / ,` | Pitch bend slower / faster (hold) |

| Keys | Action |
|---|---|
| `V / N` | Crossfader left / right · with ⇧ all the way |
| `B` | Crossfader to the centre |
| `?` | Shortcut overview |
| `⌘F` | Search the library |
| `⌘L` | Track list (there Enter, `1` / `2` load) |
| `⌘,` | Settings |
| `⌘⌥M` | MIDI monitor |

## 🚀 Getting started

**Requirements:** macOS 13 or newer (Apple Silicon recommended), Rust stable with the Xcode
Command Line Tools, Node.js 22 and pnpm 10.

```bash
git clone https://github.com/pepperonas/rille.git
cd rille
pnpm install          # frontend dependencies, including the local Tauri CLI
pnpm tauri dev        # build and start the app
```

| Command | What it does |
|---|---|
| `pnpm tauri dev` | Start the app (Vite on :1420 + Rust, hot reload) |
| `pnpm dev` | UI only, in the browser, against a simulated backend (`?demo` for a lively state) |
| `pnpm check` | Everything CI runs: build, typecheck, ESLint, Vitest, rustfmt, Clippy, cargo test |
| `pnpm e2e` | Playwright tests against the simulated backend (uses the installed Chrome) |
| `pnpm badges` | Recompute the README numbers (`.github/badges/*.json`) |
| `pnpm screenshots` | Re-render `docs/screenshots/*` and `docs/banner.png` |
| `pnpm perf:ui` | Measure frame times of the interface (two decks playing, then idle) |
| `pnpm tokens` | Regenerate the colour tokens after changing a seed colour |
| `pnpm tauri build` | Build `rille.app` / `.dmg` (unsigned for now) |

## 🔊 Audio engine

- **Realtime rules.** The CoreAudio callback never allocates, locks, logs or does I/O. A test
  renders 2,000 blocks under [`assert_no_alloc`](https://crates.io/crates/assert_no_alloc) —
  and a counter-check proves the guard would notice an allocation. Debug builds of the app abort
  loudly if the callback ever allocates.
- **Lock-free in and out.** The UI and the controller each have their own
  [`rtrb`](https://crates.io/crates/rtrb) command queue; the engine publishes its state through a
  triple buffer (newest wins, no back-pressure). Replaced tracks travel back through a garbage
  queue, so memory is never freed on the audio thread.
- **Testable without a device.** `Engine::process(&mut out, channels)` knows nothing about
  cpal; tests render offline and measure.
- **Smoothing everywhere.** Faders and crossfader are smoothed per sample (time-based), so
  moves never zipper. Curves: channel fader squared (−12 dB at half), crossfader constant power
  / linear / cut.
- **Declick.** Stops, starts, cue jumps and seeks become 4 ms fades or crossfades.
- **Playhead and rate.** Each deck plays from a fractional playhead at a smoothed rate (tempo ×
  bend, negative in reverse, the platter speed while scratching). Without keylock the track is
  read with 4-point Hermite interpolation; with keylock Signalsmith Stretch gets exactly
  `rate × block` input frames per block. Its C++ code is checked for allocations through the
  macOS `malloc_logger` hook (which, unlike `assert_no_alloc`, also sees C++): zero in
  `process`, `reset` and `seek`.
- **Device changes.** A wait-free `EngineSlot` hands the engine from one output stream to the
  next, so loaded tracks, positions and cue points survive a device switch. cpal reports errors
  partly on the render thread and every CoreAudio overload as an "xrun" error — rille only
  touches atomics there, counts overloads, and rebuilds the stream only when it is really
  broken (falling back to the default device if the old one vanished).
- **Decoding** with [symphonia](https://github.com/pdeljanov/Symphonia), resampling with
  [rubato](https://github.com/HEnquist/rubato) (FFT), bounded to 1 h / 200 M frames and
  8–384 kHz so a broken header cannot exhaust memory.

## 🏛️ Architecture

```
 React UI ──commands──▶ src-tauri ──rtrb──▶ ┌──────────────────────────┐
    ▲                     │                 │ rille-engine             │ CoreAudio
    │ 60 Hz state frames  │ AudioService    │ Engine::process (RT)     ├──────────▶ output
    └──── Tauri channel ◀─┤ bridge thread ◀─┤ triple buffer · events   │
                          │                 └──────────────────────────┘
 DDJ-200 ◀── LEDs ──── ControllerService ──rtrb──▶ (own command queue)
   USB-MIDI ───────────▶ rille-midi: table → decoder → soft takeover → mapping
                          rille-library: decode (symphonia) · resample (rubato)
```

| Thread | Job | Allowed |
|---|---|---|
| Main (AppKit/Tauri) | windows, commands, first CoreMIDI client | everything except blocking |
| Audio (CoreAudio) | `Engine::process`, queues, snapshot | lock- and allocation-free work only |
| Bridge (60 Hz) | state frames to the UI, garbage, stream health | normal |
| Controller | MIDI in → engine commands, engine state → LEDs, hotplug | normal |
| Loader | decode + resample, hand tracks to the engine | normal |

- **IPC:** typed Tauri commands for actions; a `Channel` streams state frames at up to 60 Hz and
  only while something changes or plays (0 Hz when idle). The UI extrapolates positions between
  frames, so time readouts run at display rate (120 Hz on ProMotion).
- **Security:** strict CSP (no remote content, fonts bundled), minimal Tauri capabilities.
- The frontend never computes anything audio-relevant; a mock backend with the same interface
  runs the UI in a normal browser for development and tests.

## 🎨 Design

- **Material 3 Expressive**, dark first. Colours are generated from one seed plus one accent
  per deck (cyan / orange) with
  [material-color-utilities](https://github.com/material-foundation/material-color-utilities);
  a test checks text contrast (≥ 4.5:1) and accent visibility (≥ 3:1) in both themes.
- **Tokens are the single source of truth** (`src/design/tokens.css`); a test fails if a
  component stylesheet contains a raw pixel value or colour.
- **Motion** uses springs only; effect springs never overshoot (tested), and
  `prefers-reduced-motion` turns animations off.
- **Roboto Flex** variable font, bundled locally; emphasized width for times and titles.
- **Private by design:** no network access, no tracking, no account.

## 📏 Performance

Measured on an M1 Pro, release builds, with heavy unrelated load on the machine (load average
14–17). Full protocol with every command: [`docs/performance.md`](docs/performance.md).

| Budget | Target | Measured |
|---|---|---|
| Buffer | 256 frames @ 48 kHz without dropouts | 0 of 5,625 blocks over budget (two keylocked decks, 30 s); slowest 64 % of 5.3 ms |
| Playback while the library analyses | no block over budget | 8 workers, 797 analyses in 30 s: 0 over budget, slowest 16 % |
| Engine, keylock off / on | low | 0.4 % / 2.0 % of one core on average |
| Slowest block with keylock | within the block's time | 692 µs = 13 % at 256 frames, 61 % at 64 |
| Track load | < 300 ms until playable | playable after 1–2 ms, first second after 4 ms; full decode of a 6:50 MP3 in 720 ms in the background |
| Analysis | background | 0.96 s for a 6:50 MP3 on one core |
| Controller → audible | < 10 ms (without device latency) | MIDI to decoded action: median 43 µs, p99 83 µs; plus at most one block (5.3 ms) |
| Interface, two decks playing | smooth | 60 fps (headless cap), p99 frame interval 16.8 ms, 0 long tasks |
| Interface, idle | draw nothing | 0 waveform draws per second |
| App idle | minimal | 0.0–0.8 % of one core (release, output open) |
| Memory per track | acceptable | 23 MB per minute (48 kHz stereo f32) |

## 🧪 Testing

The badges at the top are measured, not typed: `tools/stats.mjs` counts the tests and lines,
and CI commits new numbers whenever they change.

| Suite | Where | Highlights |
|---|---|---|
| Rust unit + integration | `crates/*/src`, `crates/*/tests`, `src-tauri/src` | cue state machine, curves, smoothing, declick, meters, measured EQ/filter frequency responses, echo-out/filter-out life cycle, keylock pitch and timing at ±16 % (zero crossings), keylock alignment with the playhead, sync at ×½/×2 and range widening, sync lock holding the phase for 30 s while the leader changes tempo, loops exact to the frame at any tempo, headphone bus before the fader, clock drift of ±200 ppm followed for 3 minutes without a dropout, no gap after jumps at any tempo, reverse, scratch, bend, search, decoder edge cases, every PDF row of the DDJ-200, 14-bit/jog/shift decoding, soft takeover incl. reconnect and fast sweeps, progressive loading (reader never sees unpublished frames), BPM on synthetic patterns (kicks at 90–174 BPM, swing, open hats, breakdown, live-drummer drift) and refusals (silence, noise, random hits), peaks format, database migrations, import → analysis → peaks end to end |
| Allocation guard | `crates/rille-engine/tests/no_alloc.rs` | 2,000 blocks under `assert_no_alloc` incl. tempo, keylock, scratch and reverse, plus a counter-check that a planted allocation is caught |
| C++ allocation guard | `crates/rille-engine/tests/stretch_alloc.rs` | Signalsmith Stretch `process`/`reset`/`seek` at changing rates through the `malloc_logger` hook, with a counter-check |
| CoreMIDI round trip | `crates/rille-midi/tests/virtual_roundtrip.rs` | virtual DDJ-200 → port → decoder → action, and LEDs back |
| Frontend (Vitest) | `src/**/*.test.ts`, `scripts`, `tools` | peaks format (byte-identical with Rust), zoom level choice, beat positions, library search/sort/selection, store and load races, position extrapolation, token sync and contrast, "no raw values" guard |
| E2E (Playwright) | `e2e/` | keyboard shortcuts (deck, pads, crossfader, held keys let go on blur, quiet while typing or in dialogs, list keys don't hit the pads), light / system theme and saved preferences, no-device notice, hot cues, loops, beat jump, sync and lock, grid editor and tap, decks fit above the library at 1024×680–1440×900, waveforms drawn (checked on a screenshot), no drawing while idle, click-to-seek, zoom, remaining-time warning with and without reduced motion, library import with progress, virtualised list, search, sort, keyboard loading, drag onto a deck, load, play, cue, eject, keyboard operation, faders, tempo range and readout, keylock, reverse and bend while held, layout at 1024×680, settings, controller UI — against the mock backend |

Practices: Clippy with `-D warnings`, `unwrap`/`expect`/`panic` denied outside tests, and new
guards are mutation-checked (the test is run once against deliberately broken code to prove it
fails).

```bash
pnpm check   # what CI runs
pnpm e2e     # browser tests
```

## 🗂️ Project structure

```
rille/
├── crates/
│   ├── rille-core/      shared types: DeckId, Command, Snapshot, TrackAudio
│   ├── rille-engine/    realtime engine, DSP, cpal output, EngineSlot
│   ├── rille-midi/      DDJ-200 table, decoder, mapping, soft takeover, LEDs, ports, virtual DDJ
│   └── rille-library/   decoding, resampling, scan, tags, BPM/beatgrid, peaks, SQLite
├── src-tauri/           app shell: AudioService, ControllerService, commands, bridge
├── src/                 React UI: deck/, mixer/, library/, waveform/, settings/, dev/, design/, ipc/, state/
├── e2e/                 Playwright tests
├── tools/               stats (badges), screenshots, banner and window frame
├── scripts/             colour token generator
├── docs/                banner, screenshots, plans (docs/superpowers/plans)
└── .github/             CI and badge data
```

## 🗺️ Roadmap

| Milestone | Content | Status |
|---|---|---|
| M0 | Foundation: workspace, Tauri shell, tokens, CI | ✅ |
| M1 | Engine core: playback, cue, mixer, meters, devices | ✅ |
| M2 | Pioneer DDJ-200: mapping, LEDs, takeover, hotplug, monitor | ✅ (hardware acceptance pending) |
| M3 | Isolator EQ with kill, bipolar filter, transition FX (echo-out, filter-out) | ✅ |
| M4 | Tempo ranges, keylock (Signalsmith Stretch), pitch bend, scratch, reverse | ✅ (jog calibration at hardware acceptance) |
| M5 | Library: import, SQLite, BPM/beatgrid analysis, search, drag & drop | ✅ |
| M6 | Waveforms (overview + scrolling detail, frequency colours), deck UI | ✅ |
| M7 | Sync & sync lock, hot cues, beat loops, beat jump, pad modes | ✅ (hardware acceptance pending) |
| M8 | Headphone cueing on a second device or a 4-channel interface | ✅ (hardware acceptance pending) |
| M9 | Full keyboard control, light theme, empty states, measured budgets | ✅ |

Later: key detection, more effects, recording, sampler pads.

## 🧯 Troubleshooting

**No sound / "Kein Audio-Gerät".** Open the settings and pick an output. rille runs at 48 kHz
(44.1 kHz if the default device cannot); a device that supports neither reports a clear error.

**The DDJ-200 is not detected.** Use a data-capable USB cable and check that the controller
appears in *Audio MIDI Setup → MIDI Studio*. rille polls once per second; the MIDI monitor
(⌘⌥M) shows whether messages arrive.

**Dropouts.** The dropout counter is in the settings. Raise the buffer size (512 or 1024), and
close apps that use the same audio device.

**A fader on the controller does nothing.** Soft takeover: move it past the value shown in the
app, then it takes over.

## 🤝 Contributing

Issues and pull requests are welcome. Please read [`CLAUDE.md`](CLAUDE.md) and the rules in
[`.claude/rules/`](.claude/rules) (realtime audio, MIDI mapping, design) first; run
`pnpm check` and `pnpm e2e` before opening a PR. Commits follow
[Conventional Commits](https://www.conventionalcommits.org). No code from GPL projects (for
example the Mixxx mappings) may be copied into rille.

Changes are listed in the [changelog](CHANGELOG.md).

## ❤️ Support

rille is free, open source and built in the evenings by one person. If it makes your sets
better, a coffee or a review helps a lot:

<a href="https://www.paypal.com/donate/?business=martin.pfeffer@celox.io&currency_code=EUR&item_name=rille"><img src="https://img.shields.io/badge/Donate-PayPal-00457C?logo=paypal&logoColor=white&style=for-the-badge" alt="Donate via PayPal"></a>
<a href="https://g.page/r/CXgdRV3QysvxEBM/review"><img src="https://img.shields.io/badge/Rate%20celox.io-Google%20Maps%20%E2%AD%90-4285F4?logo=googlemaps&logoColor=white&style=for-the-badge" alt="Rate celox.io on Google Maps"></a>

- ☕ **Donate** via PayPal: [martin.pfeffer@celox.io](https://www.paypal.com/donate/?business=martin.pfeffer@celox.io&currency_code=EUR&item_name=rille)
- ⭐ **Rate celox.io** on Google Maps: [g.page/r/CXgdRV3QysvxEBM/review](https://g.page/r/CXgdRV3QysvxEBM/review)
- 🌟 Star the repo, report issues, send PRs — just as welcome.

## 📄 License

[MIT](LICENSE) © 2026 Martin Pfeffer · [celox.io](https://celox.io)

**Third-party components:** Roboto Flex (SIL Open Font License 1.1),
material-color-utilities (Apache 2.0, build time only), and the Rust and npm dependencies
listed in `Cargo.lock` and `pnpm-lock.yaml` under their own licenses. Pioneer DJ and DDJ-200 are
trademarks of their respective owners; rille is not affiliated with or endorsed by them.
