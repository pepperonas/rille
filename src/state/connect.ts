import { backend } from '../ipc';
import type { Deck } from '../ipc/types';
import {
  frameReceived,
  loadAccepted,
  loadFailed,
  loadRequested,
  loadWarning,
  nextLoadSeq,
  loaded,
  store,
  unloaded,
  updateLibrary,
} from './store';
import { importPaths, libraryError, loadSelected, moveSelected } from '../library/actions';
import { mergeTracks, mergeUnder } from '../library/model';
import { initPrefs } from '../settings/applyPrefs';

/** Wire backend streams into the store. Returns a cleanup function. */
export async function connectBackend(): Promise<() => void> {
  // Events win over the initial fetch: it may have been read before them.
  let progressSeen = false;
  const offs = await Promise.all([
    backend.subscribeState((frame) =>
      store.update((s) => frameReceived(s, frame, performance.now())),
    ),
    backend.on('deck-loaded', (e) => store.update((s) => loaded(s, e))),
    backend.on('deck-load-failed', (e) => store.update((s) => loadFailed(s, e))),
    backend.on('deck-load-warning', (e) => store.update((s) => loadWarning(s, e))),
    backend.on('audio-changed', (audio) => store.update((s) => ({ ...s, audio }))),
    backend.on('controller-changed', (controller) => store.update((s) => ({ ...s, controller }))),
    backend.on('library-tracks', (tracks) =>
      store.update((s) =>
        updateLibrary(s, (l) => ({ ...l, tracks: mergeTracks(l.tracks, tracks) })),
      ),
    ),
    backend.on('library-progress', (progress) => {
      progressSeen = true;
      // A running import means the library works again; drop an old error.
      store.update((s) =>
        updateLibrary(s, (l) => ({ ...l, progress, error: progress.importing ? null : l.error })),
      );
    }),
    backend.on('library-scroll', (rows) => moveSelected(rows)),
    backend.on('library-load', (deck) => loadSelected(deck)),
    backend.onFileDrop(({ paths, x, y }) => {
      const hit = document.elementFromPoint(x, y);
      // Onto the library: import (folders too). Onto a deck: play the first file.
      if (hit?.closest('[data-library]')) {
        importPaths(paths);
        return;
      }
      const deck = hit?.closest<HTMLElement>('[data-deck]')?.dataset.deck;
      const path = paths[0];
      if ((deck === 'a' || deck === 'b') && path) void loadIntoDeck(deck, path);
    }),
  ]);
  void Promise.all([backend.libraryTracks(), backend.libraryProgress()]).then(
    ([tracks, progress]) =>
      store.update((s) =>
        updateLibrary(s, (l) => ({
          ...l,
          tracks: mergeUnder(l.tracks, tracks),
          progress: progressSeen ? l.progress : progress,
          ready: true,
        })),
      ),
    (e: unknown) => {
      libraryError(e);
      store.update((s) => updateLibrary(s, (l) => ({ ...l, ready: true })));
    },
  );
  const [audio, controller] = await Promise.all([
    backend.audioStatus(),
    backend.controllerStatus().catch(() => null),
  ]);
  store.update((s) => ({ ...s, audio, controller }));
  if (backend.kind === 'mock' && new URLSearchParams(location.search).has('demo')) {
    // Browser demo (screenshots): two tracks, loaded through the normal path.
    void loadIntoDeck('a', '/demo/Deep Hours (Extended Mix).wav');
    void loadIntoDeck('b', '/demo/Neon Avenue (Club Edit).wav');
  }
  const offPrefs = initPrefs();
  return () => {
    offPrefs();
    offs.forEach((off) => off());
  };
}

export async function loadIntoDeck(deck: Deck, path: string): Promise<void> {
  const title =
    path
      .split('/')
      .pop()
      ?.replace(/\.[^.]+$/, '') ?? path;
  const seq = nextLoadSeq();
  store.update((s) => loadRequested(s, deck, title, seq));
  try {
    const id = await backend.loadFile(deck, path);
    store.update((s) => loadAccepted(s, deck, id, seq));
  } catch (e) {
    store.update((s) =>
      loadFailed(s, {
        deck,
        trackId: null,
        title,
        message: e instanceof Error ? e.message : String(e),
      }),
    );
  }
}

export function unloadDeck(deck: Deck) {
  backend.deck(deck, { type: 'unload' });
  store.update((s) => unloaded(s, deck));
}
