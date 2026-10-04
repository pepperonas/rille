// Shapes exchanged with the Rust side. Must match src-tauri/src/dto.rs (camelCase JSON).

export type Deck = 'a' | 'b';
export type Curve = 'smooth' | 'linear' | 'cut';

export interface DeckFrame {
  trackId: number | null;
  position: number;
  frames: number;
  playing: boolean;
  cue: number;
  previewing: boolean;
  peak: [number, number];
}

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
  | { type: 'unload' };

export type MixerAction =
  | { type: 'channelFader'; deck: Deck; value: number }
  | { type: 'trim'; deck: Deck; value: number }
  | { type: 'crossfader'; value: number }
  | { type: 'curve'; curve: Curve }
  | { type: 'masterGain'; value: number };

export interface ControllerStatus {
  connected: boolean;
  name: string | null;
  vinylMode: boolean;
}

export interface MonitorLine {
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
  deck(deck: Deck, action: DeckAction): void;
  mixer(action: MixerAction): void;
}
