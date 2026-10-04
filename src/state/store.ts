import { useSyncExternalStore } from 'react';
import type {
  AudioStatus,
  ControllerStatus,
  Deck,
  DeckLoadFailed,
  DeckLoaded,
  StateFrame,
} from '../ipc/types';

export type DeckInfo =
  | { status: 'empty' }
  | { status: 'loading'; requestId: number | null; title: string }
  | { status: 'ready'; trackId: number; title: string; durationSecs: number }
  | { status: 'error'; title: string; message: string };

export interface AppState {
  frame: StateFrame | null;
  /** `performance.now()` when `frame` arrived; base for position extrapolation. */
  receivedAt: number;
  decks: Record<Deck, DeckInfo>;
  audio: AudioStatus | null;
  controller: ControllerStatus | null;
}

export const initialState: AppState = {
  frame: null,
  receivedAt: 0,
  decks: { a: { status: 'empty' }, b: { status: 'empty' } },
  audio: null,
  controller: null,
};

// Pure transitions, unit-tested.

export function loadRequested(s: AppState, deck: Deck, title: string): AppState {
  return { ...s, decks: { ...s.decks, [deck]: { status: 'loading', requestId: null, title } } };
}

export function loadAccepted(s: AppState, deck: Deck, requestId: number): AppState {
  const d = s.decks[deck];
  if (d.status !== 'loading') return s;
  return { ...s, decks: { ...s.decks, [deck]: { ...d, requestId } } };
}

/** Does a result for `trackId` belong to the load the deck is currently waiting for? */
function awaited(d: DeckInfo, trackId: number | null): boolean {
  return d.status === 'loading' && (d.requestId === null || trackId === null || d.requestId === trackId);
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
    durationSecs: e.durationSecs,
  };
  return { ...s, decks: { ...s.decks, [e.deck]: info } };
}

export function loadFailed(s: AppState, e: DeckLoadFailed): AppState {
  const d = s.decks[e.deck];
  if (!awaited(d, e.trackId)) return s;
  const info: DeckInfo = { status: 'error', title: e.title, message: e.message };
  return { ...s, decks: { ...s.decks, [e.deck]: info } };
}

export function unloaded(s: AppState, deck: Deck): AppState {
  return { ...s, decks: { ...s.decks, [deck]: { status: 'empty' } } };
}

export function frameReceived(s: AppState, frame: StateFrame, now: number): AppState {
  return { ...s, frame, receivedAt: now };
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
