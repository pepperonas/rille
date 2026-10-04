// In-browser stand-in for the Rust side (`pnpm dev:web`, Playwright). Simulates two decks with
// synthetic tracks so the UI can be developed and tested without Tauri. Not used in the app.
import type {
  AudioStatus,
  Backend,
  BackendEvents,
  Deck,
  DeckAction,
  DeckFrame,
  MixerAction,
  StateFrame,
} from './types';

const RATE = 48_000;
const TRACK_SECONDS = 210;

type MockDeck = DeckFrame;

function emptyDeck(): MockDeck {
  return {
    trackId: null,
    position: 0,
    frames: 0,
    playing: false,
    cue: 0,
    previewing: false,
    peak: [0, 0],
  };
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
    decks.forEach((d, i) => {
      if (d.playing) {
        d.position = Math.min(d.frames, d.position + frames);
        if (d.position >= d.frames) {
          d.playing = false;
          d.previewing = false;
          emit('deck-ended', i === 0 ? 'a' : 'b');
        }
      }
      const level = d.playing ? 0.35 + 0.3 * Math.abs(Math.sin(now / 230 + i)) : 0;
      const gain = (state.channelFader[i] ?? 1) ** 2;
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
      xruns: 0,
      droppedCommands: 0,
    };
  }

  function deckAction(d: MockDeck, action: DeckAction) {
    if (d.frames === 0 && action.type !== 'unload') return;
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
      case 'unload':
        Object.assign(d, emptyDeck());
        break;
    }
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
      Object.assign(d, emptyDeck(), { trackId: id, frames: TRACK_SECONDS * RATE });
      const title = path.split('/').pop()?.replace(/\.[^.]+$/, '') ?? 'Unbenannt';
      const preset = demo ? DEMO_TRACKS[title] : undefined;
      if (preset) {
        d.position = Math.round(preset[0] * RATE);
        d.cue = Math.round(16.2 * RATE);
        d.playing = true;
        state.channelFader[idx(deck)] = preset[1];
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
    deck(deck, action) {
      deckAction(decks[idx(deck)], action);
    },
    mixer: mixerAction,
  };
}
