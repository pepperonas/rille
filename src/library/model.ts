// Library state and its pure transitions: merging backend updates, search, sort, selection.
import type { Deck, LibraryProgress, LibraryTrack } from '../ipc/types';

export type SortKey = 'title' | 'artist' | 'bpm' | 'duration' | 'added';
export type SortDir = 'asc' | 'desc';

export interface Sort {
  key: SortKey;
  dir: SortDir;
}

export interface LibraryState {
  /** In import order (by id). */
  tracks: LibraryTrack[];
  progress: LibraryProgress;
  query: string;
  sort: Sort;
  selected: number | null;
  /** The list has been fetched once (distinguishes "empty" from "not loaded yet"). */
  ready: boolean;
  /** Why the library cannot be used or an import failed; shown in the panel. */
  error: string | null;
  /** Bumped when the selection moved on purpose (keys, controller, search); the list scrolls
   *  it into view only then, not when background updates shift its row. */
  reveal: number;
}

export const initialLibrary: LibraryState = {
  tracks: [],
  progress: { importing: false, queued: 0, analyzed: 0 },
  query: '',
  sort: { key: 'added', dir: 'asc' },
  selected: null,
  ready: false,
  error: null,
  reveal: 0,
};

/** Replace tracks with the same id, add new ones; result in id (= import) order. */
export function mergeTracks(list: LibraryTrack[], incoming: LibraryTrack[]): LibraryTrack[] {
  if (incoming.length === 0) return list;
  const byId = new Map(list.map((t) => [t.id, t]));
  for (const t of incoming) byId.set(t.id, t);
  return [...byId.values()].sort((a, b) => a.id - b.id);
}

/** The initial fetch, slid underneath what events already delivered: a row from an event is
 *  never older than the fetched one (analysis only moves forward), so it wins. */
export function mergeUnder(current: LibraryTrack[], fetched: LibraryTrack[]): LibraryTrack[] {
  return mergeTracks(fetched, current);
}

/** After the filter changed: keep the selection if it is still shown, else the first row. A
 *  hidden selection would let Enter or the controller load a track nobody can see. */
export function keepVisibleSelection(s: LibraryState): LibraryState {
  const visible = visibleTracks(s);
  if (visible.some((t) => t.id === s.selected)) return s;
  const selected = visible[0]?.id ?? null;
  return selected === s.selected ? s : { ...s, selected };
}

/** Lower case without accents: "Beyoncé" is found by "beyonce". */
function fold(s: string): string {
  return s.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase();
}

/** Every word of the query must appear in title, artist or album; a number also matches the
 *  rounded BPM ("128" finds 127.9 BPM tracks). */
export function matches(track: LibraryTrack, query: string): boolean {
  const words = fold(query).split(/\s+/).filter(Boolean);
  if (words.length === 0) return true;
  const text = fold([track.title, track.artist ?? '', track.album ?? ''].join(' '));
  const bpm = track.bpm === null ? '' : String(Math.round(track.bpm));
  return words.every((w) => text.includes(w) || (/^\d+$/.test(w) && bpm === w));
}

const collator = new Intl.Collator('de', { sensitivity: 'base', numeric: true });

function compare(a: LibraryTrack, b: LibraryTrack, key: SortKey): number {
  switch (key) {
    case 'title':
      return collator.compare(a.title, b.title);
    case 'artist':
      return collator.compare(a.artist ?? '', b.artist ?? '');
    case 'bpm':
      return (a.bpm ?? 0) - (b.bpm ?? 0);
    case 'duration':
      return (a.duration ?? 0) - (b.duration ?? 0);
    case 'added':
      return a.id - b.id;
  }
}

/** Is the sort value missing? Those rows go last in either direction. */
function missing(t: LibraryTrack, key: SortKey): boolean {
  if (key === 'bpm') return t.bpm === null;
  if (key === 'duration') return t.duration === null;
  if (key === 'artist') return !t.artist;
  return false;
}

export function sortTracks(tracks: LibraryTrack[], sort: Sort): LibraryTrack[] {
  const sign = sort.dir === 'asc' ? 1 : -1;
  return [...tracks].sort((a, b) => {
    const ma = missing(a, sort.key);
    const mb = missing(b, sort.key);
    if (ma !== mb) return ma ? 1 : -1;
    return sign * compare(a, b, sort.key) || collator.compare(a.title, b.title) || a.id - b.id;
  });
}

let cache: { tracks: LibraryTrack[]; query: string; sort: Sort; result: LibraryTrack[] } | null =
  null;

/** The rows as shown: filtered and sorted. Cached for the last inputs (rendering and the
 *  controller ask for it often, the inputs change rarely). */
export function visibleTracks(s: LibraryState): LibraryTrack[] {
  if (cache && cache.tracks === s.tracks && cache.query === s.query && cache.sort === s.sort) {
    return cache.result;
  }
  const result = sortTracks(
    s.tracks.filter((t) => matches(t, s.query)),
    s.sort,
  );
  cache = { tracks: s.tracks, query: s.query, sort: s.sort, result };
  return result;
}

/** Move the selection by `delta` rows, clamped. With nothing (visible) selected, the first
 *  step lands on the first row going down and on the last going up. */
export function moveSelection(
  visible: LibraryTrack[],
  selected: number | null,
  delta: number,
): number | null {
  if (visible.length === 0) return null;
  const at = visible.findIndex((t) => t.id === selected);
  const next =
    at < 0
      ? delta >= 0
        ? 0
        : visible.length - 1
      : Math.max(0, Math.min(visible.length - 1, at + delta));
  return visible[next]?.id ?? null;
}

/** Clicking a column: same column flips the direction, a new one starts ascending. */
export function nextSort(sort: Sort, key: SortKey): Sort {
  if (sort.key === key) return { key, dir: sort.dir === 'asc' ? 'desc' : 'asc' };
  return { key, dir: 'asc' };
}

/** Where Enter / double-click loads: the first empty deck, else deck 1. */
export function targetDeck(empty: Record<Deck, boolean>): Deck {
  if (empty.a) return 'a';
  if (empty.b) return 'b';
  return 'a';
}

/** "Analysiere 12 von 40" while work is running; null when idle. */
export function progressText(p: LibraryProgress): string | null {
  if (p.queued > p.analyzed) return `Analysiere ${p.analyzed} von ${p.queued}`;
  if (p.importing) return 'Importiere …';
  return null;
}

export function formatDuration(secs: number | null): string {
  if (secs === null || !Number.isFinite(secs)) return '–';
  const s = Math.round(secs);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
}

export function formatBpm(bpm: number | null): string {
  return bpm === null ? '–' : bpm.toFixed(1);
}

const indexes = new WeakMap<LibraryTrack[], Map<number, LibraryTrack>>();

/** Track by id without scanning the list on every frame (one index per list version). */
export function trackById(tracks: LibraryTrack[], id: number | null): LibraryTrack | undefined {
  if (id === null) return undefined;
  let index = indexes.get(tracks);
  if (!index) {
    index = new Map(tracks.map((t) => [t.id, t]));
    indexes.set(tracks, index);
  }
  return index.get(id);
}
