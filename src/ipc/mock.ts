// In-browser stand-in for the Rust side (`pnpm dev:web`, Playwright). Simulates two decks with
// synthetic tracks so the UI can be developed and tested without Tauri. Not used in the app.
import type {
  AudioStatus,
  Band,
  Backend,
  BackendEvents,
  Deck,
  DeckAction,
  DeckFrame,
  Grid,
  LibraryProgress,
  LibraryTrack,
  MixerAction,
  StateFrame,
} from './types';
import { NEXT_RANGE, RANGE_SPAN } from '../state/extrapolate';
import { encodePeaks, syntheticPeaks } from '../waveform/peaks';
import {
  beatAt,
  beatFrames,
  follow,
  frameOfBeat,
  quantizeCue,
  tempoForRate,
  wrapHalf,
} from '../deck/beat';

const RATE = 48_000;
const BANDS: Band[] = ['low', 'mid', 'high'];
const TRACK_SECONDS = 210;

/** Bend held from the app, as in the engine (`BEND_HOLD`). */
const BEND_HOLD = 0.04;

type MockDeck = DeckFrame & { bend: -1 | 0 | 1; libraryId: number | null };

function emptyDeck(): MockDeck {
  return {
    trackId: null,
    position: 0,
    frames: 0,
    playing: false,
    cue: 0,
    previewing: false,
    peak: [0, 0],
    tempo: 0,
    tempoRange: 'ten',
    keylock: false,
    rate: 1,
    reverse: false,
    scratching: false,
    grid: null,
    hotCues: Array<number | null>(8).fill(null),
    loopRange: null,
    syncLock: false,
    bend: 0,
    libraryId: null,
  };
}

/** Rate the engine would play at: tempo within the range, bend, direction. */
function mockRate(d: MockDeck): number {
  const base = (1 + d.tempo * RANGE_SPAN[d.tempoRange]) * (1 + d.bend * BEND_HOLD);
  return d.reverse ? -base : base;
}

/** Fake controller traffic for the MIDI monitor in demo mode. */
const DEMO_MIDI: Array<[boolean, string, string | null]> = [
  [false, 'B0 21 42', 'Deck 1 · Jog-Rand +2'],
  [false, 'B0 21 41', 'Deck 1 · Jog-Rand +1'],
  [false, 'B6 1F 30', null],
  [false, 'B6 3F 12', 'Mixer · Crossfader 37.6 %'],
  [false, '91 0B 7F', 'Deck 2 · Play/Pause ↓'],
  [true, '91 0B 7F', null],
  [false, '91 0B 00', 'Deck 2 · Play/Pause ↑'],
  [false, 'B1 13 7A', null],
  [false, 'B1 33 40', 'Deck 2 · Kanalfader 96.1 %'],
  [false, '90 3F 7F', 'Deck 1 · Shift ↓'],
  [false, 'B0 21 3E', 'Deck 1 · Jog-Rand + Shift -2'],
  [false, '90 3F 00', 'Deck 1 · Shift ↑'],
  [true, '90 0C 7F', null],
  [true, '9F 00 7F', null],
];

const ARTISTS = ['Ada Lux', 'Nachtfahrt', 'Kollektiv 9', 'Mira Vale', 'Øresund', 'Dub Atlas'];
const WORDS = ['Neon', 'Avenue', 'Deep', 'Hours', 'Static', 'Bloom', 'Tide', 'Signal', 'Velvet'];

/** Deterministic fake tracks; `analyzed` false leaves them pending. */
export function fakeTracks(from: number, count: number, analyzed: boolean): LibraryTrack[] {
  return Array.from({ length: count }, (_, i) => {
    const n = from + i;
    const title = `${WORDS[n % WORDS.length]} ${WORDS[(n * 5 + 3) % WORDS.length]}${n > 8 ? ` ${n}` : ''}`;
    const artist = ARTISTS[(n * 7) % ARTISTS.length] ?? 'Unbekannt';
    return {
      id: n,
      path: `/Musik/${artist}/${title}.mp3`,
      title,
      artist,
      album: null,
      duration: 180 + ((n * 37) % 240),
      bpm: analyzed ? fakeBpm(n) : null,
      firstBeat: analyzed && fakeBpm(n) !== null ? 0.05 + (n % 7) * 0.031 : null,
      analysis: analyzed ? 'done' : 'pending',
      error: null,
      analysisRev: analyzed ? 1 : 0,
    };
  });
}

const gridOf = (t: LibraryTrack): Grid | null =>
  t.bpm !== null && t.firstBeat !== null ? { bpm: t.bpm, firstBeat: t.firstBeat } : null;

function fakeBpm(n: number): number | null {
  if (n % 11 === 7) return null; // ambient: no clear tempo
  return 118 + ((n * 13) % 300) / 10;
}

export function createMockBackend(): Backend {
  const decks: [MockDeck, MockDeck] = [emptyDeck(), emptyDeck()];
  const state = {
    channelFader: [1, 1] as [number, number],
    trim: [0.5, 0.5] as [number, number],
    crossfader: 0.5,
    curve: 'smooth' as StateFrame['curve'],
    masterGain: 1,
    frameClock: 0,
    eq: [
      [0.5, 0.5, 0.5],
      [0.5, 0.5, 0.5],
    ] as StateFrame['eq'],
    eqKill: [
      [false, false, false],
      [false, false, false],
    ] as StateFrame['eqKill'],
    filter: [0.5, 0.5] as [number, number],
    cue: [false, false] as [boolean, boolean],
    masterCue: false,
    cueMix: 0.5,
    cueLevel: 0.8,
    transition: { kind: 'echoOut', deck: null, releasing: false } as StateFrame['transition'],
    /** performance.now() when the running transition started. */
    transitionStart: 0,
  };
  let nextId = 1;
  let library: LibraryTrack[] = [];
  const progress: LibraryProgress = { importing: false, queued: 0, analyzed: 0 };
  // `?demo`: both decks loaded and playing, a controller "connected" — for screenshots.
  const demo = typeof location !== 'undefined' && new URLSearchParams(location.search).has('demo');
  const controller = {
    connected: demo,
    name: demo ? 'DDJ-200' : (null as string | null),
    vinylMode: true,
  };
  // `?noaudio`: start without an output device (empty-state tests); choosing one connects.
  const noAudio =
    typeof location !== 'undefined' && new URLSearchParams(location.search).has('noaudio');
  let onFrame: ((f: StateFrame) => void) | null = null;
  const listeners = new Map<string, Set<(p: unknown) => void>>();
  const status: AudioStatus = {
    connected: !noAudio,
    deviceId: 'mock',
    deviceName: demo ? 'MacBook Pro-Lautsprecher' : 'Browser (Simulation)',
    sampleRate: RATE,
    channels: 2,
    requestedBuffer: 256,
    bufferFrames: 256,
    latencyMs: 5.3,
    deviceXruns: 0,
    error: noAudio ? 'Das Ausgabegerät ist nicht mehr da.' : null,
    cue: {
      setting: { kind: 'off' },
      active: false,
      deviceName: null,
      bufferFrames: null,
      latencyMs: 0,
      underruns: 0,
      error: null,
    },
  };

  const emit = <E extends keyof BackendEvents>(event: E, payload: BackendEvents[E]) =>
    listeners.get(event)?.forEach((l) => l(payload));

  if (demo) library = fakeTracks(1, 64, true);

  /** Import as the Rust side does: rows first (pending), then one analysis result at a time. */
  const simulateImport = (count: number) => {
    const from = library.length + 1;
    const added = fakeTracks(from, count, false);
    library = [...library, ...added];
    Object.assign(progress, { importing: true });
    emit('library-progress', { ...progress });
    setTimeout(() => {
      emit('library-tracks', added);
      Object.assign(progress, { importing: false, queued: progress.queued + count });
      emit('library-progress', { ...progress });
      added.forEach((t, i) =>
        setTimeout(
          () => {
            const bpm = fakeBpm(t.id);
            const done: LibraryTrack = {
              ...t,
              analysis: 'done',
              analysisRev: t.analysisRev + 1,
              bpm,
              firstBeat: bpm === null ? null : 0.05,
            };
            library = library.map((x) => (x.id === t.id ? done : x));
            progress.analyzed += 1;
            emit('library-tracks', [done]);
            if (progress.analyzed === progress.queued) {
              emit('library-progress', { ...progress });
              Object.assign(progress, { queued: 0, analyzed: 0 });
            } else emit('library-progress', { ...progress });
          },
          120 * (i + 1),
        ),
      );
    }, 80);
  };

  const idx = (d: Deck) => (d === 'a' ? 0 : 1);

  let last = performance.now();
  let ticking = false;
  const tick = () => {
    const now = performance.now();
    const frames = Math.round(((now - last) / 1000) * RATE);
    last = now;
    state.frameClock += frames;
    // Simulated transition: runs ~3 s (release ~0.5 s), then pauses its deck.
    const t = state.transition;
    if (t.deck) {
      const age = now - state.transitionStart;
      if (t.releasing ? age > 500 : age > 3000) {
        if (!t.releasing) decks[idx(t.deck)].playing = false;
        state.transition = { ...t, deck: null, releasing: false };
      }
    }
    decks.forEach((d, i) => {
      const other = decks[1 - i];
      // Sync lock (simplified): follow the other deck's tempo, snap the phase.
      if (d.syncLock && other?.playing) syncTo(d, other, true);
      d.rate = mockRate(d);
      if (d.playing) {
        const before = d.position;
        d.position = Math.max(0, Math.min(d.frames, d.position + frames * d.rate));
        // Only crossing the loop end loops back (a jump beyond it has left the loop).
        if (d.loopRange && !d.reverse && before < d.loopRange[1] && d.position >= d.loopRange[1]) {
          const [start, end] = d.loopRange;
          d.position = start + ((d.position - end) % Math.max(1, end - start));
        }
        if (d.reverse && d.position <= 0) d.playing = false;
        if (d.position >= d.frames) {
          d.playing = false;
          d.previewing = false;
          emit('deck-ended', i === 0 ? 'a' : 'b');
        }
      }
      const level = d.playing ? 0.35 + 0.3 * Math.abs(Math.sin(now / 230 + i)) : 0;
      const fx = state.transition.deck === (i === 0 ? 'a' : 'b') && !state.transition.releasing;
      const fxGain = fx ? Math.max(0, 1 - (now - state.transitionStart) / 3000) : 1;
      const gain = (state.channelFader[i] ?? 1) ** 2 * fxGain;
      d.peak = [level * gain, level * gain * 0.95];
    });
    onFrame?.(frame());
  };

  function frame(): StateFrame {
    const strip = (d: MockDeck): DeckFrame => ({
      trackId: d.trackId,
      position: d.position,
      frames: d.frames,
      playing: d.playing,
      cue: d.cue,
      previewing: d.previewing,
      peak: d.peak,
      tempo: d.tempo,
      tempoRange: d.tempoRange,
      keylock: d.keylock,
      rate: d.rate,
      reverse: d.reverse,
      scratching: d.scratching,
      grid: d.grid ? { ...d.grid } : null,
      hotCues: [...d.hotCues],
      loopRange: d.loopRange ? [d.loopRange[0], d.loopRange[1]] : null,
      syncLock: d.syncLock,
    });
    const peak = Math.max(decks[0].peak[0], decks[1].peak[0]) * state.masterGain;
    return {
      frameClock: state.frameClock,
      sampleRate: RATE,
      decks: [strip(decks[0]), strip(decks[1])],
      channelFader: [...state.channelFader],
      trim: [...state.trim],
      crossfader: state.crossfader,
      curve: state.curve,
      masterGain: state.masterGain,
      masterPeak: [peak, peak],
      eq: [[...state.eq[0]], [...state.eq[1]]],
      eqKill: [[...state.eqKill[0]], [...state.eqKill[1]]],
      filter: [...state.filter],
      cue: [...state.cue],
      masterCue: state.masterCue,
      cueMix: state.cueMix,
      cueLevel: state.cueLevel,
      cuePeak: status.cue.active
        ? [
            Math.max(state.cue[0] ? decks[0].peak[0] : 0, state.cue[1] ? decks[1].peak[0] : 0),
            Math.max(state.cue[0] ? decks[0].peak[1] : 0, state.cue[1] ? decks[1].peak[1] : 0),
          ]
        : [0, 0],
      transition: { ...state.transition },
      xruns: 0,
      droppedCommands: 0,
    };
  }

  function deckAction(d: MockDeck, action: DeckAction) {
    // Transport needs a track; tempo, range and keylock can be set ahead (as in the engine).
    const settings = [
      'unload',
      'tempo',
      'tempoRange',
      'cycleTempoRange',
      'keylock',
      'bend',
      'reverse',
    ];
    if (d.frames === 0 && !settings.includes(action.type)) return;
    const before = d.position;
    switch (action.type) {
      case 'play':
        d.previewing = false;
        if (d.position < d.frames) d.playing = true;
        break;
      case 'pause':
        d.playing = false;
        d.previewing = false;
        break;
      case 'playPause':
        if (d.playing && !d.previewing) d.playing = false;
        else deckAction(d, { type: 'play' });
        break;
      case 'cuePress':
        if (d.playing && !d.previewing) {
          d.position = d.cue;
          d.playing = false;
        } else if (d.position !== d.cue) {
          d.cue = d.position;
        } else if (d.position < d.frames) {
          d.playing = true;
          d.previewing = true;
        }
        break;
      case 'cueRelease':
        if (d.previewing) {
          d.previewing = false;
          d.playing = false;
          d.position = d.cue;
        }
        break;
      case 'jumpToStart':
        d.position = 0;
        break;
      case 'jumpToCue':
        d.position = d.cue;
        d.playing = false;
        d.previewing = false;
        break;
      case 'seek':
        d.position = Math.max(0, Math.min(d.frames, action.frame));
        break;
      case 'unload': {
        // Tempo, range and keylock stay with the deck, like on a CDJ.
        const { tempo, tempoRange, keylock } = d;
        Object.assign(d, emptyDeck(), { tempo, tempoRange, keylock }); // sync lock goes
        break;
      }
      case 'tempo':
        d.tempo = Number.isFinite(action.value) ? Math.max(-1, Math.min(1, action.value)) : 0;
        break;
      case 'tempoRange':
        d.tempoRange = action.range;
        break;
      case 'cycleTempoRange':
        d.tempoRange = NEXT_RANGE[d.tempoRange];
        break;
      case 'keylock':
        d.keylock = action.on;
        break;
      case 'bend':
        d.bend = action.direction;
        break;
      case 'reverse':
        d.reverse = action.on;
        break;
      case 'sync': {
        const other = decks[decks[0] === d ? 1 : 0];
        if (other) syncTo(d, other, other.playing);
        break;
      }
      case 'syncLock':
      case 'toggleSyncLock': {
        d.syncLock = action.type === 'syncLock' ? action.on : !d.syncLock;
        const other = decks[decks[0] === d ? 1 : 0];
        if (d.syncLock && other) syncTo(d, other, other.playing);
        break;
      }
      case 'beatLoop': {
        if (!d.grid || !(action.beats > 0)) break;
        const len = action.beats * beatFrames(d.grid, RATE);
        if (d.loopRange) {
          const [start, end] = d.loopRange;
          if (Math.abs(end - start - len) < 1) d.loopRange = null;
          else if (start + len < d.frames) {
            d.loopRange = [start, start + len];
            if (d.position >= start + len) d.position = start + ((d.position - start) % len);
          }
          break;
        }
        const step = Math.min(1, action.beats);
        const at = beatAt(d.grid, d.position, RATE) / step;
        const n = at - Math.floor(at) > 0.875 ? Math.ceil(at) : Math.floor(at);
        const start = Math.max(0, frameOfBeat(d.grid, n * step, RATE));
        if (start + len < d.frames) d.loopRange = [start, start + len];
        break;
      }
      case 'loopExit':
        d.loopRange = null;
        break;
      case 'beatJump': {
        if (!d.grid) break;
        const delta = action.beats * beatFrames(d.grid, RATE);
        const to = Math.max(0, Math.min(d.frames - 1, d.position + delta));
        if (d.loopRange) {
          const [start, end] = [d.loopRange[0] + delta, d.loopRange[1] + delta];
          if (start < 0 || end > d.frames - 1) break; // would push the loop off the track
          d.loopRange = [start, end];
        }
        d.position = to;
        break;
      }
      case 'hotCue': {
        const cue = d.hotCues[action.slot];
        if (cue === undefined) break;
        if (cue === null)
          d.hotCues[action.slot] = Math.round(quantizeCue(d.grid, d.position, RATE));
        else d.position = cue;
        break;
      }
      case 'hotCueClear':
        if (action.slot < d.hotCues.length) d.hotCues[action.slot] = null;
        break;
    }
    // Jumping out of a loop leaves it, as in the engine.
    if (d.loopRange && d.position !== before) {
      const [start, end] = d.loopRange;
      if (d.position < start || d.position >= end) d.loopRange = null;
    }
    d.rate = mockRate(d);
  }

  /** The engine's sync rules: tempo (×½/×1/×2, widening the range), phase if wanted. */
  function syncTo(d: MockDeck, other: MockDeck, phase: boolean) {
    if (!d.grid || !other.grid || other.frames === 0) return;
    const target = other.grid.bpm * (1 + other.tempo * RANGE_SPAN[other.tempoRange]);
    const f = follow(d.grid.bpm, target);
    const t = f && tempoForRate(f.rate, d.tempoRange);
    if (!f || !t) return;
    d.tempoRange = t.range;
    d.tempo = t.tempo;
    if (!phase || !d.playing) return;
    const error = wrapHalf(
      beatAt(d.grid, d.position, RATE) -
        beatAt(other.grid, other.position, RATE) * f.beatsPerTargetBeat,
    );
    d.position = Math.max(0, d.position - error * beatFrames(d.grid, RATE));
  }

  function mixerAction(action: MixerAction) {
    const unit = (v: number) => (Number.isFinite(v) ? Math.min(1, Math.max(0, v)) : 0);
    switch (action.type) {
      case 'channelFader':
        state.channelFader[idx(action.deck)] = unit(action.value);
        break;
      case 'trim':
        state.trim[idx(action.deck)] = unit(action.value);
        break;
      case 'crossfader':
        state.crossfader = unit(action.value);
        break;
      case 'curve':
        state.curve = action.curve;
        break;
      case 'masterGain':
        state.masterGain = unit(action.value);
        break;
      case 'eq':
        state.eq[idx(action.deck)][BANDS.indexOf(action.band)] = unit(action.value);
        break;
      case 'eqKill':
        state.eqKill[idx(action.deck)][BANDS.indexOf(action.band)] = action.kill;
        break;
      case 'filter':
        state.filter[idx(action.deck)] = unit(action.value);
        break;
      case 'transitionFx': {
        const t = state.transition;
        if (t.deck) {
          state.transition = { ...t, releasing: true };
          state.transitionStart = performance.now();
          break;
        }
        const loudest = ([0, 1] as const)
          .filter((i) => decks[i].playing)
          .map((i) => ({
            i,
            level:
              (state.channelFader[i] ?? 0) ** 2 *
              (i === 0 ? 1 - state.crossfader : state.crossfader),
          }))
          .sort((a, b) => b.level - a.level)[0];
        if (loudest) {
          state.transition = { ...t, deck: loudest.i === 0 ? 'a' : 'b', releasing: false };
          state.transitionStart = performance.now();
        }
        break;
      }
      case 'cycleTransitionFx':
        if (!state.transition.deck) {
          state.transition = {
            ...state.transition,
            kind: state.transition.kind === 'echoOut' ? 'filterOut' : 'echoOut',
          };
        }
        break;
      case 'headphoneCue':
        state.cue[idx(action.deck)] = action.on;
        break;
      case 'toggleHeadphoneCue':
        state.cue[idx(action.deck)] = !state.cue[idx(action.deck)];
        break;
      case 'masterCue':
        state.masterCue = action.on;
        break;
      case 'cueMix':
        state.cueMix = unit(action.value);
        break;
      case 'cueLevel':
        state.cueLevel = unit(action.value);
        break;
    }
  }

  /** Demo positions for the screenshot tracks: (seconds, channel fader). */
  /** Demo presets: position (s), channel fader, artist, tempo. */
  const DEMO_TRACKS: Record<string, [number, number, string, number]> = {
    'Deep Hours (Extended Mix)': [74.3, 0.86, 'Mira Vale', 122.0],
    'Neon Avenue (Club Edit)': [191.6, 0.92, 'Nachtfahrt', 124.0],
  };

  function startDemo() {
    state.crossfader = 0.42;
    emit('controller-changed', { ...controller });
  }

  const load = (
    deck: Deck,
    title: string,
    artist: string | null,
    bpm: number | null,
    durationSecs: number,
    libraryId: number | null,
    grid: Grid | null = null,
  ): number => {
    const id = nextId++;
    const d = decks[idx(deck)];
    // Tempo, range and keylock stay with the deck across loads, as in the engine.
    const { tempo, tempoRange, keylock, syncLock } = d;
    Object.assign(d, emptyDeck(), {
      trackId: id,
      libraryId,
      grid,
      syncLock,
      frames: Math.round(durationSecs * RATE),
      tempo,
      tempoRange,
      keylock,
    });
    const preset = demo ? DEMO_TRACKS[title] : undefined;
    if (preset) {
      d.position = Math.round(preset[0] * RATE);
      d.cue = Math.round(16.2 * RATE);
      d.playing = true;
      state.channelFader[idx(deck)] = preset[1];
      // Deck 1 runs slightly faster with keylock, so the screenshots show the tempo section.
      if (deck === 'a') {
        d.tempo = 0.24;
        d.keylock = true;
      }
    }
    setTimeout(
      () => emit('deck-loaded', { deck, trackId: id, title, artist, bpm, libraryId, durationSecs }),
      50,
    );
    return id;
  };

  return {
    kind: 'mock',
    async subscribeState(handler) {
      onFrame = handler;
      if (!ticking) {
        ticking = true;
        last = performance.now();
        setInterval(tick, 1000 / 60);
        if (demo) startDemo();
      }
      return () => {
        // Only drop our own subscription; a newer subscriber may already have replaced it.
        if (onFrame === handler) onFrame = null;
      };
    },
    async on(event, handler) {
      const set = listeners.get(event) ?? new Set();
      listeners.set(event, set);
      const l = handler as (p: unknown) => void;
      set.add(l);
      return () => set.delete(l);
    },
    async onFileDrop() {
      return () => {};
    },
    async audioDevices() {
      return [
        { id: 'mock', name: 'Browser (Simulation)', maxChannels: 2, isDefault: true },
        { id: 'mock-phones', name: 'USB-Kopfhörer', maxChannels: 2, isDefault: false },
      ];
    },
    async audioStatus() {
      return { ...status };
    },
    async setCue(setting) {
      // Simulated devices: the main output has two outputs, a second device always works.
      if (setting.kind === 'channels34') {
        status.cue = {
          ...status.cue,
          setting,
          active: false,
          deviceName: null,
          error:
            'Das Ausgabegerät hat keine Ausgänge 3/4. Wähle ein Gerät mit vier Ausgängen oder ein zweites Gerät für den Kopfhörer.',
        };
        emit('audio-changed', { ...status, cue: { ...status.cue } });
        throw new Error(status.cue.error ?? '');
      }
      status.cue =
        setting.kind === 'off'
          ? {
              ...status.cue,
              setting,
              active: false,
              deviceName: null,
              bufferFrames: null,
              latencyMs: 0,
              error: null,
            }
          : {
              ...status.cue,
              setting,
              active: true,
              deviceName:
                setting.deviceId === 'mock-phones' ? 'USB-Kopfhörer' : 'MacBook Pro-Kopfhörer',
              bufferFrames: setting.bufferFrames,
              latencyMs: (setting.bufferFrames * 3) / (RATE / 1000),
              error: null,
            };
      emit('audio-changed', { ...status, cue: { ...status.cue } });
      return { ...status, cue: { ...status.cue } };
    },
    async audioOpen(_deviceId, bufferFrames) {
      status.connected = true;
      status.error = null;
      status.requestedBuffer = bufferFrames;
      status.bufferFrames = bufferFrames;
      status.latencyMs = (bufferFrames / RATE) * 1000;
      emit('audio-changed', { ...status });
      return { ...status };
    },
    async libraryTracks() {
      return [...library];
    },
    async libraryProgress() {
      return { ...progress };
    },
    async libraryImport(paths) {
      simulateImport(Math.max(1, paths.length) * 4);
    },
    async libraryPeaks(id) {
      const t = library.find((x) => x.id === id);
      if (!t || t.analysis !== 'done') throw new Error('Waveform wird berechnet');
      return encodePeaks(
        syntheticPeaks(t.duration ?? TRACK_SECONDS, t.bpm ?? 120, id, t.firstBeat ?? 0),
      );
    },
    async libraryChooseFolder() {
      simulateImport(12);
    },
    async loadTrack(deck, id) {
      const t = library.find((x) => x.id === id);
      if (!t) throw new Error('Dieser Track ist nicht mehr in der Library');
      return load(deck, t.title, t.artist, t.bpm, t.duration ?? TRACK_SECONDS, t.id, gridOf(t));
    },
    async loadFile(deck, path) {
      const title =
        path
          .split('/')
          .pop()
          ?.replace(/\.[^.]+$/, '') ?? 'Unbenannt';
      // As in the app, a dropped file joins the library (here: analysed at once).
      let t = library.find((x) => x.path === path);
      if (!t) {
        const preset = demo ? DEMO_TRACKS[title] : undefined;
        t = {
          id: library.reduce((m, x) => Math.max(m, x.id), 0) + 1,
          path,
          title,
          artist: preset?.[2] ?? null,
          album: null,
          duration: TRACK_SECONDS,
          bpm: preset?.[3] ?? 124,
          firstBeat: 0.12,
          analysis: 'done',
          error: null,
          analysisRev: 1,
        };
        library = [...library, t];
        emit('library-tracks', [t]);
      }
      return load(deck, t.title, t.artist, t.bpm, t.duration ?? TRACK_SECONDS, t.id, gridOf(t));
    },
    async controllerStatus() {
      return { ...controller };
    },
    async setPadMode() {},
    async setGrid(id, bpm, firstBeat) {
      const t = library.find((x) => x.id === id);
      if (!t) throw new Error('Dieser Track ist nicht mehr in der Library');
      const grid = bpm > 0 ? { bpm, firstBeat } : null;
      const changed: LibraryTrack = {
        ...t,
        bpm: grid?.bpm ?? null,
        firstBeat: grid?.firstBeat ?? null,
      };
      library = library.map((x) => (x.id === id ? changed : x));
      // The real library sends its events in batches every 250 ms.
      setTimeout(() => emit('library-tracks', [changed]), 250);
      for (const d of decks) if (d.libraryId === id) d.grid = grid;
    },
    async setVinylMode(on) {
      controller.vinylMode = on;
      emit('controller-changed', { ...controller });
    },
    async setMidiMonitor(on) {
      if (!on || !demo) return;
      let seq = 0;
      let t = 1_200_000;
      const batch = () =>
        DEMO_MIDI.map(([outgoing, hex, meaning]) => {
          t += 9_000 + (seq % 5) * 3_100;
          return { seq: seq++, timeUs: t, outgoing, hex, meaning };
        });
      emit('midi-monitor', [...batch(), ...batch()]);
    },
    async openExternal(url) {
      window.open(url, '_blank', 'noopener');
    },
    async setWindowTheme(theme) {
      // Visible to the browser tests; there is no native window here.
      document.documentElement.dataset.windowTheme = theme ?? 'system';
    },
    deck(deck, action) {
      deckAction(decks[idx(deck)], action);
    },
    mixer: mixerAction,
  };
}
