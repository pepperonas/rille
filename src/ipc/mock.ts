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
  MixerAction,
  StateFrame,
} from './types';
import { NEXT_RANGE, RANGE_SPAN } from '../state/extrapolate';

const RATE = 48_000;
const BANDS: Band[] = ['low', 'mid', 'high'];
const TRACK_SECONDS = 210;

/** Bend held from the app, as in the engine (`BEND_HOLD`). */
const BEND_HOLD = 0.04;

type MockDeck = DeckFrame & { bend: -1 | 0 | 1 };

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
    bend: 0,
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
    transition: { kind: 'echoOut', deck: null, releasing: false } as StateFrame['transition'],
    /** performance.now() when the running transition started. */
    transitionStart: 0,
  };
  let nextId = 1;
  // `?demo`: both decks loaded and playing, a controller "connected" — for screenshots.
  const demo = typeof location !== 'undefined' && new URLSearchParams(location.search).has('demo');
  const controller = {
    connected: demo,
    name: demo ? 'DDJ-200' : (null as string | null),
    vinylMode: true,
  };
  let onFrame: ((f: StateFrame) => void) | null = null;
  const listeners = new Map<string, Set<(p: unknown) => void>>();
  const status: AudioStatus = {
    connected: true,
    deviceId: 'mock',
    deviceName: demo ? 'MacBook Pro-Lautsprecher' : 'Browser (Simulation)',
    sampleRate: RATE,
    channels: 2,
    requestedBuffer: 256,
    bufferFrames: 256,
    latencyMs: 5.3,
    deviceXruns: 0,
    error: null,
  };

  const emit = <E extends keyof BackendEvents>(event: E, payload: BackendEvents[E]) =>
    listeners.get(event)?.forEach((l) => l(payload));

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
      d.rate = mockRate(d);
      if (d.playing) {
        d.position = Math.max(0, Math.min(d.frames, d.position + frames * d.rate));
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
      transition: { ...state.transition },
      xruns: 0,
      droppedCommands: 0,
    };
  }

  function deckAction(d: MockDeck, action: DeckAction) {
    // Transport needs a track; tempo, range and keylock can be set ahead (as in the engine).
    const settings = ['unload', 'tempo', 'tempoRange', 'cycleTempoRange', 'keylock', 'bend', 'reverse'];
    if (d.frames === 0 && !settings.includes(action.type)) return;
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
        Object.assign(d, emptyDeck(), { tempo, tempoRange, keylock });
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
    }
    d.rate = mockRate(d);
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
          .map((i) => ({ i, level: (state.channelFader[i] ?? 0) ** 2 * (i === 0 ? 1 - state.crossfader : state.crossfader) }))
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
    }
  }

  /** Demo positions for the screenshot tracks: (seconds, channel fader). */
  const DEMO_TRACKS: Record<string, [number, number]> = {
    'Deep Hours (Extended Mix)': [74.3, 0.86],
    'Neon Avenue (Club Edit)': [191.6, 0.92],
  };

  function startDemo() {
    state.crossfader = 0.42;
    emit('controller-changed', { ...controller });
  }

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
      return [{ id: 'mock', name: 'Browser (Simulation)', maxChannels: 2, isDefault: true }];
    },
    async audioStatus() {
      return { ...status };
    },
    async audioOpen(_deviceId, bufferFrames) {
      status.requestedBuffer = bufferFrames;
      status.bufferFrames = bufferFrames;
      status.latencyMs = (bufferFrames / RATE) * 1000;
      emit('audio-changed', { ...status });
      return { ...status };
    },
    async loadFile(deck, path) {
      const id = nextId++;
      const d = decks[idx(deck)];
      // Tempo, range and keylock stay with the deck across loads, as in the engine.
      const { tempo, tempoRange, keylock } = d;
      Object.assign(d, emptyDeck(), {
        trackId: id,
        frames: TRACK_SECONDS * RATE,
        tempo,
        tempoRange,
        keylock,
      });
      const title = path.split('/').pop()?.replace(/\.[^.]+$/, '') ?? 'Unbenannt';
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
        () => emit('deck-loaded', { deck, trackId: id, title, durationSecs: TRACK_SECONDS }),
        50,
      );
      return id;
    },
    async controllerStatus() {
      return { ...controller };
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
    deck(deck, action) {
      deckAction(decks[idx(deck)], action);
    },
    mixer: mixerAction,
  };
}
