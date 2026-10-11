// What the library UI (and the controller) can do. State lives in the app store.
import { backend } from '../ipc';
import type { Deck, LibraryTrack } from '../ipc/types';
import { loadAccepted, loadFailed, loadRequested, store, updateLibrary } from '../state/store';
import { moveSelection, nextSort, targetDeck, visibleTracks, type SortKey } from './model';

export function selectTrack(id: number | null) {
  store.update((s) => updateLibrary(s, (l) => (l.selected === id ? l : { ...l, selected: id })));
}

export function setQuery(query: string) {
  store.update((s) => updateLibrary(s, (l) => ({ ...l, query })));
}

export function sortBy(key: SortKey) {
  store.update((s) => updateLibrary(s, (l) => ({ ...l, sort: nextSort(l.sort, key) })));
}

/** Move the selection over the rows as shown (keyboard, controller). */
export function moveSelected(rows: number) {
  store.update((s) =>
    updateLibrary(s, (l) => {
      const selected = moveSelection(visibleTracks(l), l.selected, rows);
      return selected === l.selected ? l : { ...l, selected };
    }),
  );
}

export async function loadTrack(deck: Deck, track: LibraryTrack): Promise<void> {
  store.update((s) => loadRequested(s, deck, track.title));
  try {
    const id = await backend.loadTrack(deck, track.id);
    store.update((s) => loadAccepted(s, deck, id));
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

function selectedTrack(): LibraryTrack | undefined {
  const { library } = store.get();
  return library.tracks.find((t) => t.id === library.selected);
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

export function chooseFolder() {
  backend.libraryChooseFolder().catch((e: unknown) => console.error('import', e));
}

export function importPaths(paths: string[]) {
  backend.libraryImport(paths).catch((e: unknown) => console.error('import', e));
}
