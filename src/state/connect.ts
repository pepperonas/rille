import { backend } from '../ipc';
import type { Deck } from '../ipc/types';
import {
  frameReceived,
  loadAccepted,
  loadFailed,
  loadRequested,
  loaded,
  store,
  unloaded,
} from './store';

/** Wire backend streams into the store. Returns a cleanup function. */
export async function connectBackend(): Promise<() => void> {
  const offs = await Promise.all([
    backend.subscribeState((frame) =>
      store.update((s) => frameReceived(s, frame, performance.now())),
    ),
    backend.on('deck-loaded', (e) => store.update((s) => loaded(s, e))),
    backend.on('deck-load-failed', (e) => store.update((s) => loadFailed(s, e))),
    backend.on('audio-changed', (audio) => store.update((s) => ({ ...s, audio }))),
    backend.onFileDrop(({ paths, x, y }) => {
      const target = document.elementFromPoint(x, y)?.closest<HTMLElement>('[data-deck]');
      const deck = target?.dataset.deck;
      const path = paths[0];
      if ((deck === 'a' || deck === 'b') && path) void loadIntoDeck(deck, path);
    }),
  ]);
  const audio = await backend.audioStatus();
  store.update((s) => ({ ...s, audio }));
  return () => offs.forEach((off) => off());
}

export async function loadIntoDeck(deck: Deck, path: string): Promise<void> {
  const title = path.split('/').pop()?.replace(/\.[^.]+$/, '') ?? path;
  store.update((s) => loadRequested(s, deck, title));
  try {
    const id = await backend.loadFile(deck, path);
    store.update((s) => loadAccepted(s, deck, id));
  } catch (e) {
    store.update((s) =>
      loadFailed(s, { deck, title, message: e instanceof Error ? e.message : String(e) }),
    );
  }
}

export function unloadDeck(deck: Deck) {
  backend.deck(deck, { type: 'unload' });
  store.update((s) => unloaded(s, deck));
}
