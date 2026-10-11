# Changelog

All notable changes to rille are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- M8 headphones: pre-fader cue per channel, master cue, cue/master blend and level (all
  smoothed); routed to outputs 3/4 of the main device or to a second device through a
  lock-free ring with drift compensation (P controller on the ring fill, cubic resampling).
  A failing headphone device never stops the master. DDJ-200 headphone and master cue buttons
  toggle with LEDs; settings choose the headphone output and buffer.
- M7 beat features: sync (tempo ×½/×1/×2 with range widening, phase), sync lock (tempo follow
  plus a phase correction of at most 2 % speed), beat loops ¼–32 beats exact to the frame at
  any tempo, beat jump ±½/1/4/16 (moves a running loop), eight hot cues per deck snapped to the
  beat and saved in the library (schema v3), beatgrid correction (move, ÷2, ×2, ±0.01 BPM,
  tap). Pads in three modes in the app and on the DDJ-200 with LEDs (cues lit, active loop
  blinking with the beat), sync LED shows the lock, tempo set by sync goes to soft takeover.
- M6 waveforms: scrolling detail view in WebGL2 (one draw call; peaks as textures per zoom
  level, beatgrid in the shader, played part dimmed) with zoom 2–32 s by mouse wheel or + / −,
  and a Canvas2D overview with played part, cue mark and click-to-seek. Three bands in shades
  of the deck colour, resolved from CSS tokens. Drawn only while a deck plays.
- Remaining-time warning for the last 30 seconds.
- Files dropped onto a deck join the library; the analysis of a track on a deck jumps the
  queue; missing cached peaks are recomputed on demand (`library_peaks`).
- M5 library: import folders (button, or drop from the Finder onto the library), tags from
  ID3/Vorbis/MP4 with an `Artist - Title` file-name fallback, SQLite database with versioned
  migrations (refuses a newer schema instead of damaging it), changed files are re-analysed.
- Background analysis on `cores − 2` workers at QoS utility: BPM and beatgrid (spectral flux,
  autocorrelation with octave check, phase-folding refinement, kick-band downbeat phase,
  consistency check that rejects noise and random hits instead of guessing) and three-band
  waveform peaks at four zoom levels (binary format "RLPK" v1, cached per track).
- Progressive loading: a track is playable ~4 ms after loading starts and fills while it plays
  (lock-free published-frame counter); a newer load or an eject cancels the decode.
- Library UI: virtualised list, search (title, artist, album, BPM; accents ignored), sorting,
  analysis progress, loading by double-click, Enter, `1` / `2`, pointer drag onto a deck; decks
  show artist and BPM. DDJ-200: Shift + jog rim browses, Shift + headphone cue loads.
- `examples/analyze.rs`, `examples/decode.rs`, `examples/load_test.rs` (rille-library).
- M4 tempo: tempo fader per deck with ranges ±6 / ±10 / ±16 % / wide (±50 %), top = slower as
  printed on the DDJ-200; varispeed via 4-point Hermite interpolation. Keylock with Signalsmith
  Stretch (latency-compensated, a varispeed bridge covers its start-up after jumps), pitch bend
  (jog rim and held − / + buttons), scratch in vinyl mode, Shift + platter search, reverse while
  held. DDJ-200: tempo fader with soft takeover, Shift + Sync cycles the range.
- Deck UI: tempo fader with range chip and percent readout, KEY, REV and bend buttons; the time
  readout extrapolates with the real playback rate (also backwards).
- `tests/stretch_alloc.rs`: proves through the macOS `malloc_logger` hook that Signalsmith
  Stretch does not allocate in `process`, `reset` and `seek`.
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

### Changed
- Track loading no longer decodes the whole file first (was ~360 ms for a 3:43 MP3).

### Fixed
- Headphones (review of M8): switching the headphones or a setting while nothing plays now
  reaches the app (the frame filter compared fields one by one and also missed EQ, kill,
  filter and transition changes since M3 – it now compares the whole state); a device switch
  starts at the target latency instead of with a backlog of up to 340 ms; clearing the ring
  can no longer swap left and right; the headphone device opens without blocking the bridge;
  concurrent switches cannot leave route and stream inconsistent; the headphones are set up
  again after every change of the main output and refuse the main device itself; large clock
  drifts (1000 ppm) are followed too (integral action, scaled with the buffer size); the
  dropout count is read without touching the audio callback and starts at 0 per device.
- Beat features (review of M7): a jump past an active loop (hot cue, cue, seek) leaves the
  loop instead of landing somewhere inside it; loops that would reach the track end are not
  made (the track used to stop with the loop still shown); a beat jump never pushes a loop off
  the track or shrinks it; a loop pressed just before a beat starts on that beat; SYNC on a
  paused deck matches the tempo but keeps the cue position; unloading releases sync lock; a
  corrected or tapped grid survives re-analysis (schema v4); quick grid-editor clicks all
  count; hot cues no longer drift by a frame per save; soft takeover sees tempo changes made
  by sync shortly after the fader was touched.
- Since M6 the decks were taller than the stage below ~880 px window height and covered the
  top of the library; they now grow denser there (sizes from tokens), checked at 1024×680,
  1280×800 and 1440×900.
- Quick arrow presses on the overview built on the position before the previous seek.
- Waveforms (review of M6): a trackpad swipe zooms step by step instead of racing to the end;
  a track without a waveform says why (analysis failed, no library) instead of "wird
  berechnet" forever; a file changed on disk and loaded again gets a fresh analysis; peaks that
  cannot be saved mark the analysis failed instead of re-running it forever; the GPU context
  survives sleep / GPU switches and is released on every track change; loading from the Finder
  no longer blocks the window; beat lines have their intended strength and stop at the track
  ends; moving the window to a screen with another pixel density re-renders sharply; the
  remaining-time warning only fires while the deck plays (at its real speed) and frames the
  overview; the overview is a keyboard-operable slider and never seeks onto the last frame.
- Library (review of M5): rows delivered by events are no longer overwritten by a slower
  initial fetch; Enter and the controller load only a track the filter shows; a file changed
  during its own analysis is analysed again instead of keeping the stale result; a failed
  analysis is retried on the next import; analysis results are sent in batches (every 250 ms)
  instead of one re-sort per track; the list no longer jumps back to the selection while
  background updates move it; an unreadable database is moved aside and replaced, and the
  panel says why the library is unavailable.
- Decoding: a file longer than its header (beyond the reserve) is reported on the deck instead
  of being cut silently; NaN samples no longer silence the rest of a waveform; peak bins cover
  the same time at every sample rate; two loads on one deck within one round trip can no
  longer leave it loading forever.
- Keylock: pausing, jumping, reversing, touching the platter or switching KEY off no longer
  clicks — the stretcher fades out over 4 ms instead of being cut. After reverse or a scratch,
  keylock re-engages only once the rate has settled, so its audio stays aligned with the
  playhead (was up to 29 ms behind).
- Scratch speed is measured over the last 20 ms instead of one audio block, so slow drags with
  small buffers no longer jump between standstill and several times the real speed.
- DDJ-200: with vinyl mode off, touching the platter no longer stops the music; switching vinyl
  mode off or unplugging the controller mid-gesture releases a held scratch or reverse; reverse
  also ends when Shift is let go before Play.
- Two jumps within one audio block now crossfade from what was actually heard.
- App: the right mouse button no longer starts a bend or reverse, holding − and + together
  hands back to the other one on release, and a value the engine never echoes (two quick
  toggles) no longer hides later changes from the controller.
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
