# Changelog

All notable changes to rille are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- M3 mixer DSP: three-band isolator EQ per channel (Linkwitz-Riley 4th order at 200 Hz /
  2 kHz, flat when neutral, kill per band, up to +6 dB), bipolar channel filter (TPT
  state-variable filter, bit-exact bypass in the centre), and transition FX on the louder
  playing deck — Echo-Out and Filter-Out, cancel by pressing again, deck pauses afterwards.
- DDJ-200: EQ knobs, CFX and Transition FX (+ Shift to change the effect) are live; the
  Transition FX LED lights while the effect runs and blinks while a cancelled tail decays.
- Knob component (drag, fine drag with Shift, wheel, keyboard, double-click to centre).
- README with banner, screenshots and self-updating badges (version, lines of code, test
  code, unit tests per suite); German translation in `README.de.md`.
- `pnpm badges` (`tools/stats.mjs`) measures the badge numbers; CI refreshes
  `.github/badges/*.json` on every push to `main`.
- `pnpm screenshots` (`tools/screenshots.mjs`) renders the README screenshots and the social
  banner from the in-browser demo (`?demo`).
- Browser demo mode: two playing decks, a "connected" controller and simulated MIDI traffic.
- About dialog: app icon, version, "Spenden via PayPal" and "celox.io bewerten" buttons, links
  to the source code and celox.io. External links go through the Tauri opener plugin, scoped
  to exactly these URLs (a test mirrors the plugin's glob matching, including lookalike hosts).

### Fixed
- Transition FX: loading or starting a track on the deck while its effect runs now cancels the
  effect (before, the new track stayed muted and was paused when the echoes ended); cancelling
  Filter-Out during its final fade no longer jumps in level.
- Knobs and faders step from the last value sent, so key repeat and trackpad scrolling
  accumulate; the wheel is scaled by gesture size and no longer scrolls the mixer; EQ kill
  survives quick double clicks; the Transition FX button shows when a cancelled effect is only
  decaying.
- MIDI monitor: the view now subscribes before switching the stream on, so the first batch of
  messages is no longer lost.

## [0.1.0] - 2026-10-04

First development snapshot: milestones M0 (foundation), M1 (engine core) and M2 (controller).

### Added
- Cargo workspace (`rille-core`, `rille-engine`, `rille-midi`, `rille-library`) and the Tauri 2
  shell with title bar overlay, vibrancy, strict CSP and minimal capabilities.
- Realtime engine: two decks, Pioneer-style cue logic, channel/master faders, crossfader with
  three curves, peak meters, parameter smoothing, 4 ms declick on stop/start/jump, wait-free
  hand-over between output streams. The audio callback is proven allocation-free by a test.
- CoreAudio output via cpal with device and buffer selection, latency display, xrun counting,
  and automatic recovery when a device disappears.
- Decoding of MP3, AAC/M4A, ALAC, FLAC, WAV and AIFF with FFT resampling to the engine rate,
  bounded against oversized or malformed files.
- Pioneer DDJ-200 support: message table checked against the official MIDI list, 14-bit
  controls, jog, shift tracking, soft takeover, LED feedback, hotplug, vinyl-mode setting,
  MIDI monitor, and a virtual DDJ-200 for development without hardware.
- React 19 UI in Material 3 Expressive: decks, mixer, settings, generated colour tokens with
  contrast tests, spring motion, keyboard-operable controls.

[Unreleased]: https://github.com/pepperonas/rille/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/pepperonas/rille/releases/tag/v0.1.0
