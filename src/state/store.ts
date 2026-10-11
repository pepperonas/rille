import { useSyncExternalStore } from 'react';
import type {
  AudioStatus,
  ControllerStatus,
  Deck,
  DeckLoadFailed,
  DeckLoaded,
  StateFrame,
} from '../ipc/types';
import { initialLibrary, type LibraryState } from '../library/model';
import { DEFAULT_ZOOM } from '../waveform/peaks';

export type DeckInfo =
  | { status: 'empty' }
  | {
      status: 'loading';
      requestId: number | null;
      /** Which request of this app session; ties `loadAccepted` to its own request. */
      seq: number;
      title: string;
    }
  | {
      status: 'ready';
      trackId: number;
      title: string;
      artist: string | null;
      bpm: number | null;
      libraryId: number | null;
      durationSecs: number;
      /** Plays, but not all of the file could be decoded. */
      warning: string | null;
    }
  | { status: 'error'; title: string; message: string };

export interface AppState {
  frame: StateFrame | null;
  /** `performance.now()` when `frame` arrived; base for position extrapolation. */
  receivedAt: number;
  decks: Record<Deck, DeckInfo>;
  audio: AudioStatus | null;
  controller: ControllerStatus | null;
  library: LibraryState;
  /** Detail waveform zoom step (`ZOOM_SECONDS`), shared by both decks. */
  waveZoom: number;
}

export const initialState: AppState = {
  frame: null,
  receivedAt: 0,
  decks: { a: { status: 'empty' }, b: { status: 'empty' } },
  audio: null,
  controller: null,
  library: initialLibrary,
  waveZoom: DEFAULT_ZOOM,
};

// Pure transitions, unit-tested.

let lastSeq = 0;

/** A fresh sequence number for a load request. */
export function nextLoadSeq(): number {
  return ++lastSeq;
}

export function loadRequested(s: AppState, deck: Deck, title: string, seq = 0): AppState {
  return {
    ...s,
    decks: { ...s.decks, [deck]: { status: 'loading', requestId: null, seq, title } },
  };
}

/** The backend took the request and named its id. A late answer to an older request (two
 *  loads on one deck within one round trip) must not claim the newer one. */
export function loadAccepted(s: AppState, deck: Deck, requestId: number, seq = 0): AppState {
  const d = s.decks[deck];
  if (d.status !== 'loading' || d.seq !== seq) return s;
  return { ...s, decks: { ...s.decks, [deck]: { ...d, requestId } } };
}

/** Does a result for `trackId` belong to the load the deck is currently waiting for? */
function awaited(d: DeckInfo, trackId: number | null): boolean {
  return (
    d.status === 'loading' && (d.requestId === null || trackId === null || d.requestId === trackId)
  );
}

/** A finished load only wins if it is the one most recently requested for that deck and the
 *  deck was not ejected in the meantime. */
export function loaded(s: AppState, e: DeckLoaded): AppState {
  const d = s.decks[e.deck];
  if (!awaited(d, e.trackId)) return s;
  const info: DeckInfo = {
    status: 'ready',
    trackId: e.trackId,
    title: e.title,
    artist: e.artist ?? null,
    bpm: e.bpm ?? null,
    libraryId: e.libraryId ?? null,
    durationSecs: e.durationSecs,
    warning: null,
  };
  return { ...s, decks: { ...s.decks, [e.deck]: info } };
}

export function loadFailed(s: AppState, e: DeckLoadFailed): AppState {
  const d = s.decks[e.deck];
  if (!awaited(d, e.trackId)) return s;
  const info: DeckInfo = { status: 'error', title: e.title, message: e.message };
  return { ...s, decks: { ...s.decks, [e.deck]: info } };
}

export function loadWarning(s: AppState, e: DeckLoadFailed): AppState {
  const d = s.decks[e.deck];
  if (d.status !== 'ready' || d.trackId !== e.trackId) return s;
  return { ...s, decks: { ...s.decks, [e.deck]: { ...d, warning: e.message } } };
}

export function unloaded(s: AppState, deck: Deck): AppState {
  return { ...s, decks: { ...s.decks, [deck]: { status: 'empty' } } };
}

export function frameReceived(s: AppState, frame: StateFrame, now: number): AppState {
  return { ...s, frame, receivedAt: now };
}

export function updateLibrary(s: AppState, fn: (l: LibraryState) => LibraryState): AppState {
  const library = fn(s.library);
  return library === s.library ? s : { ...s, library };
}

class Store {
  private state: AppState = initialState;
  private listeners = new Set<() => void>();

  get = (): AppState => this.state;

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  update(fn: (s: AppState) => AppState) {
    const next = fn(this.state);
    if (next === this.state) return;
    this.state = next;
    this.listeners.forEach((l) => l());
  }
}

export const store = new Store();

export function useAppState<T>(select: (s: AppState) => T): T {
  return useSyncExternalStore(store.subscribe, () => select(store.get()));
}
