// What the library UI (and the controller) can do. State lives in the app store.
import { backend } from '../ipc';
import type { Deck, LibraryTrack } from '../ipc/types';
import {
  loadAccepted,
  loadFailed,
  loadRequested,
  nextLoadSeq,
  store,
  updateLibrary,
} from '../state/store';
import {
  keepVisibleSelection,
  moveSelection,
  nextSort,
  targetDeck,
  visibleTracks,
  type SortKey,
} from './model';

export function selectTrack(id: number | null) {
  store.update((s) => updateLibrary(s, (l) => (l.selected === id ? l : { ...l, selected: id })));
}

export function setQuery(query: string) {
  store.update((s) =>
    updateLibrary(s, (l) => keepVisibleSelection({ ...l, query, reveal: l.reveal + 1 })),
  );
}

export function sortBy(key: SortKey) {
  store.update((s) => updateLibrary(s, (l) => ({ ...l, sort: nextSort(l.sort, key) })));
}

/** Move the selection over the rows as shown (keyboard, controller) and bring it into view. */
export function moveSelected(rows: number) {
  store.update((s) =>
    updateLibrary(s, (l) => ({
      ...l,
      selected: moveSelection(visibleTracks(l), l.selected, rows),
      reveal: l.reveal + 1,
    })),
  );
}

export async function loadTrack(deck: Deck, track: LibraryTrack): Promise<void> {
  const seq = nextLoadSeq();
  store.update((s) => loadRequested(s, deck, track.title, seq));
  try {
    const id = await backend.loadTrack(deck, track.id);
    store.update((s) => loadAccepted(s, deck, id, seq));
  } catch (e) {
    store.update((s) =>
      loadFailed(s, {
        deck,
        trackId: null,
        title: track.title,
        message: e instanceof Error ? e.message : String(e),
      }),
    );
  }
}

/** Only a track that is shown can be loaded (the filter may hide the selection). */
function selectedTrack(): LibraryTrack | undefined {
  const { library } = store.get();
  return visibleTracks(library).find((t) => t.id === library.selected);
}

/** Load the selected track; without a deck, onto the first empty one. */
export function loadSelected(deck?: Deck) {
  const track = selectedTrack();
  if (!track) return;
  const { decks } = store.get();
  const target =
    deck ?? targetDeck({ a: decks.a.status === 'empty', b: decks.b.status === 'empty' });
  void loadTrack(target, track);
}

export function libraryError(e: unknown) {
  const error = e instanceof Error ? e.message : String(e);
  store.update((s) => updateLibrary(s, (l) => ({ ...l, error })));
}

export function chooseFolder() {
  backend.libraryChooseFolder().catch(libraryError);
}

export function importPaths(paths: string[]) {
  backend.libraryImport(paths).catch(libraryError);
}
