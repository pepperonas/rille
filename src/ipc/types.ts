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
  grid: Grid | null;
  /** Hot cues in engine frames, null for empty pads (8). */
  hotCues: Array<number | null>;
  /** Active loop [start, end) in engine frames. */
  loopRange: [number, number] | null;
  syncLock: boolean;
}

export interface Grid {
  bpm: number;
  /** Seconds. */
  firstBeat: number;
}

/** What the eight pads of a deck do (the DDJ-200 has no mode buttons). */
export type PadMode = 'hotCue' | 'beatLoop' | 'beatJump';

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
  /** Channels on the headphone bus. */
  cue: [boolean, boolean];
  masterCue: boolean;
  /** 0 = cue only, 1 = master only. */
  cueMix: number;
  cueLevel: number;
  cuePeak: [number, number];
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
  /** Headphone output. */
  cue: CueStatus;
}

/** Where the headphones are connected. */
export type CueSetting =
  | { kind: 'off' }
  /** Outputs 3/4 of the main device. */
  | { kind: 'channels34' }
  /** A second device (null = system default). */
  | { kind: 'device'; deviceId: string | null; bufferFrames: number };

export interface CueStatus {
  setting: CueSetting;
  /** The headphone signal reaches a device. */
  active: boolean;
  deviceName: string | null;
  bufferFrames: number | null;
  latencyMs: number;
  underruns: number;
  error: string | null;
}

export interface DeckLoaded {
  deck: Deck;
  trackId: number;
  title: string;
  artist: string | null;
  /** From the library analysis; null if unknown. */
  bpm: number | null;
  /** Library row (waveform, beatgrid); null when the library is unavailable. */
  libraryId: number | null;
  durationSecs: number;
}

export type AnalysisState = 'pending' | 'done' | 'failed';

/** A track in the library (src-tauri/src/library_service.rs). */
export interface LibraryTrack {
  id: number;
  path: string;
  title: string;
  artist: string | null;
  album: string | null;
  /** Seconds; null until known. */
  duration: number | null;
  /** null: not analysed yet, or no clear tempo. */
  bpm: number | null;
  /** Seconds to the first beat; with `bpm` the beatgrid. */
  firstBeat: number | null;
  analysis: AnalysisState;
  error: string | null;
  /** Changes with every finished analysis (cached peaks belong to one revision). */
  analysisRev: number;
}

export interface LibraryProgress {
  importing: boolean;
  /** Tracks queued for analysis since the queue was last idle. */
  queued: number;
  /** Of those, finished. */
  analyzed: number;
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
  | { type: 'reverse'; on: boolean }
  | { type: 'sync' }
  | { type: 'syncLock'; on: boolean }
  | { type: 'beatLoop'; beats: number }
  | { type: 'loopExit' }
  | { type: 'beatJump'; beats: number }
  | { type: 'hotCue'; slot: number }
  | { type: 'hotCueClear'; slot: number };

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
  | { type: 'cycleTransitionFx' }
  | { type: 'headphoneCue'; deck: Deck; on: boolean }
  | { type: 'masterCue'; on: boolean }
  | { type: 'cueMix'; value: number }
  | { type: 'cueLevel'; value: number };

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
  /** The track plays, but not all of the file could be decoded. */
  'deck-load-warning': DeckLoadFailed;
  'deck-ended': Deck;
  'audio-changed': AudioStatus;
  'controller-changed': ControllerStatus;
  'midi-monitor': MonitorLine[];
  /** Added or changed tracks, merged by id. */
  'library-tracks': LibraryTrack[];
  'library-progress': LibraryProgress;
  /** Controller: move the selection by this many rows. */
  'library-scroll': number;
  /** Controller: load the selected track onto this deck. */
  'library-load': Deck;
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
  /** Route the headphones; rejects with a German message if that is not possible. */
  setCue(setting: CueSetting): Promise<AudioStatus>;
  loadFile(deck: Deck, path: string): Promise<number>;
  /** Load a library track; resolves with the request id like `loadFile`. */
  loadTrack(deck: Deck, id: number): Promise<number>;
  libraryTracks(): Promise<LibraryTrack[]>;
  libraryProgress(): Promise<LibraryProgress>;
  /** Waveform peaks ("RLPK" v1); rejects while the analysis is still running. */
  libraryPeaks(id: number): Promise<ArrayBuffer>;
  /** Import dropped folders or files in the background. */
  libraryImport(paths: string[]): Promise<void>;
  /** Native folder picker, then import. */
  libraryChooseFolder(): Promise<void>;
  controllerStatus(): Promise<ControllerStatus>;
  setVinylMode(on: boolean): Promise<void>;
  /** Tell the controller what the pads of a deck do. */
  setPadMode(deck: Deck, mode: PadMode): Promise<void>;
  /** Store a corrected beatgrid (bpm ≤ 0 removes it). */
  setGrid(libraryId: number, bpm: number, firstBeat: number): Promise<void>;
  setMidiMonitor(on: boolean): Promise<void>;
  /** Open a link in the default browser (only allow-listed URLs, see links.ts). */
  openExternal(url: string): Promise<void>;
  deck(deck: Deck, action: DeckAction): void;
  mixer(action: MixerAction): void;
}
