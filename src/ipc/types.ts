// Shapes exchanged with the Rust side. Must match src-tauri/src/dto.rs (camelCase JSON).

export type Deck = 'a' | 'b';
export type Curve = 'smooth' | 'linear' | 'cut';
export type Band = 'low' | 'mid' | 'high';
export type TransitionKind = 'echoOut' | 'filterOut';

export interface TransitionFrame {
  kind: TransitionKind;
  deck: Deck | null;
  releasing: boolean;
}

export interface DeckFrame {
  trackId: number | null;
  position: number;
  frames: number;
  playing: boolean;
  cue: number;
  previewing: boolean;
  peak: [number, number];
  /** Tempo fader, -1 (top, slower) … +1 (bottom, faster). */
  tempo: number;
  tempoRange: TempoRange;
  keylock: boolean;
  /** Effective playback rate: 1 normal, negative backwards, 0 held still. */
  rate: number;
  reverse: boolean;
  scratching: boolean;
}

/** Tempo fader range: ±6 %, ±10 %, ±16 % or wide (±50 %). */
export type TempoRange = 'six' | 'ten' | 'sixteen' | 'wide';

export interface StateFrame {
  frameClock: number;
  sampleRate: number;
  decks: [DeckFrame, DeckFrame];
  channelFader: [number, number];
  trim: [number, number];
  crossfader: number;
  curve: Curve;
  masterGain: number;
  masterPeak: [number, number];
  /** EQ knob positions per deck: low, mid, high (0 kill, 0.5 unity, 1 +6 dB). */
  eq: [[number, number, number], [number, number, number]];
  eqKill: [[boolean, boolean, boolean], [boolean, boolean, boolean]];
  /** Bipolar filter per deck: 0 low-pass closed, 0.5 off, 1 high-pass open. */
  filter: [number, number];
  transition: TransitionFrame;
  xruns: number;
  droppedCommands: number;
}

export interface AudioDevice {
  id: string;
  name: string;
  maxChannels: number;
  isDefault: boolean;
}

export interface AudioStatus {
  connected: boolean;
  deviceId: string | null;
  deviceName: string | null;
  sampleRate: number;
  channels: number;
  requestedBuffer: number;
  bufferFrames: number | null;
  latencyMs: number;
  deviceXruns: number;
  error: string | null;
}

export interface DeckLoaded {
  deck: Deck;
  trackId: number;
  title: string;
  durationSecs: number;
}

export interface DeckLoadFailed {
  deck: Deck;
  trackId: number | null;
  title: string;
  message: string;
}

export type DeckAction =
  | { type: 'playPause' }
  | { type: 'play' }
  | { type: 'pause' }
  | { type: 'cuePress' }
  | { type: 'cueRelease' }
  | { type: 'jumpToStart' }
  | { type: 'jumpToCue' }
  | { type: 'seek'; frame: number }
  | { type: 'unload' }
  | { type: 'tempo'; value: number }
  | { type: 'tempoRange'; range: TempoRange }
  | { type: 'cycleTempoRange' }
  | { type: 'keylock'; on: boolean }
  /** Pitch bend held from the app: -1 slower, 0 release, +1 faster. */
  | { type: 'bend'; direction: -1 | 0 | 1 }
  /** Plays backwards while held. */
  | { type: 'reverse'; on: boolean };

export type MixerAction =
  | { type: 'channelFader'; deck: Deck; value: number }
  | { type: 'trim'; deck: Deck; value: number }
  | { type: 'crossfader'; value: number }
  | { type: 'curve'; curve: Curve }
  | { type: 'masterGain'; value: number }
  | { type: 'eq'; deck: Deck; band: Band; value: number }
  | { type: 'eqKill'; deck: Deck; band: Band; kill: boolean }
  | { type: 'filter'; deck: Deck; value: number }
  | { type: 'transitionFx' }
  | { type: 'cycleTransitionFx' };

export interface ControllerStatus {
  connected: boolean;
  name: string | null;
  vinylMode: boolean;
}

export interface MonitorLine {
  seq: number;
  timeUs: number;
  outgoing: boolean;
  hex: string;
  meaning: string | null;
}

export interface BackendEvents {
  'deck-loaded': DeckLoaded;
  'deck-load-failed': DeckLoadFailed;
  'deck-ended': Deck;
  'audio-changed': AudioStatus;
  'controller-changed': ControllerStatus;
  'midi-monitor': MonitorLine[];
}

export type Unsubscribe = () => void;

/** Files dropped onto the window; position in CSS pixels relative to the viewport. */
export interface FileDrop {
  paths: string[];
  x: number;
  y: number;
}

export interface Backend {
  readonly kind: 'tauri' | 'mock';
  subscribeState(onFrame: (frame: StateFrame) => void): Promise<Unsubscribe>;
  on<E extends keyof BackendEvents>(
    event: E,
    handler: (payload: BackendEvents[E]) => void,
  ): Promise<Unsubscribe>;
  onFileDrop(handler: (drop: FileDrop) => void): Promise<Unsubscribe>;
  audioDevices(): Promise<AudioDevice[]>;
  audioStatus(): Promise<AudioStatus>;
  audioOpen(deviceId: string | null, bufferFrames: number): Promise<AudioStatus>;
  loadFile(deck: Deck, path: string): Promise<number>;
  controllerStatus(): Promise<ControllerStatus>;
  setVinylMode(on: boolean): Promise<void>;
  setMidiMonitor(on: boolean): Promise<void>;
  /** Open a link in the default browser (only allow-listed URLs, see links.ts). */
  openExternal(url: string): Promise<void>;
  deck(deck: Deck, action: DeckAction): void;
  mixer(action: MixerAction): void;
}
