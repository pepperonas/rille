import { describe, expect, it } from 'vitest';
import type { LibraryTrack } from '../ipc/types';
import {
  keepVisibleSelection,
  mergeUnder,
  formatBpm,
  formatDuration,
  initialLibrary,
  matches,
  mergeTracks,
  moveSelection,
  nextSort,
  progressText,
  sortTracks,
  targetDeck,
  visibleTracks,
} from './model';

function track(id: number, over: Partial<LibraryTrack> = {}): LibraryTrack {
  return {
    id,
    path: `/m/${id}.mp3`,
    title: `Track ${id}`,
    artist: null,
    album: null,
    duration: 200,
    bpm: 120,
    firstBeat: 0.1,
    analysis: 'done',
    error: null,
    analysisRev: 1,
    ...over,
  };
}

describe('mergeTracks', () => {
  it('replaces by id and adds new tracks in id order', () => {
    const list = [track(1), track(3)];
    const merged = mergeTracks(list, [track(3, { bpm: 128 }), track(2)]);
    expect(merged.map((t) => t.id)).toEqual([1, 2, 3]);
    expect(merged[2]?.bpm).toBe(128);
  });

  it('keeps the same array when nothing comes in', () => {
    const list = [track(1)];
    expect(mergeTracks(list, [])).toBe(list);
  });
});

describe('matches', () => {
  const t = track(1, { title: 'Crème Brûlée', artist: 'Beyoncé', album: 'Dessert', bpm: 127.6 });

  it('needs every word, in any field, ignoring case and accents', () => {
    expect(matches(t, 'beyonce creme')).toBe(true);
    expect(matches(t, 'DESSERT')).toBe(true);
    expect(matches(t, 'beyonce pizza')).toBe(false);
    expect(matches(t, '  ')).toBe(true);
  });

  it('finds the rounded tempo by number', () => {
    expect(matches(t, '128')).toBe(true);
    expect(matches(t, '127')).toBe(false);
    expect(matches(track(2, { bpm: null }), '120')).toBe(false);
  });
});

describe('sortTracks', () => {
  const list = [
    track(1, { title: 'b', bpm: 128 }),
    track(2, { title: 'a', bpm: null }),
    track(3, { title: 'C', bpm: 90 }),
    track(4, { title: 'Track 10', bpm: 128 }),
    track(5, { title: 'Track 9', bpm: 128 }),
  ];

  it('sorts titles naturally and case-insensitively', () => {
    const ids = sortTracks(list, { key: 'title', dir: 'asc' }).map((t) => t.id);
    expect(ids).toEqual([2, 1, 3, 5, 4]);
  });

  it('puts unknown tempos last in both directions, ties by title', () => {
    expect(sortTracks(list, { key: 'bpm', dir: 'asc' }).map((t) => t.id)).toEqual([3, 1, 5, 4, 2]);
    expect(sortTracks(list, { key: 'bpm', dir: 'desc' }).map((t) => t.id)).toEqual([1, 5, 4, 3, 2]);
  });

  it('does not touch the input', () => {
    const copy = [...list];
    sortTracks(list, { key: 'title', dir: 'desc' });
    expect(list).toEqual(copy);
  });
});

describe('visibleTracks', () => {
  it('filters, sorts and caches for unchanged inputs', () => {
    const s = {
      ...initialLibrary,
      tracks: [
        track(1, { title: 'Zebra' }),
        track(2, { title: 'Apfel' }),
        track(3, { title: 'x' }),
      ],
      query: 'e',
      sort: { key: 'title' as const, dir: 'asc' as const },
    };
    const v = visibleTracks(s);
    expect(v.map((t) => t.title)).toEqual(['Apfel', 'Zebra']);
    expect(visibleTracks({ ...s })).toBe(v);
    expect(visibleTracks({ ...s, query: 'z' }).map((t) => t.title)).toEqual(['Zebra']);
  });
});

describe('moveSelection', () => {
  const v = [track(1), track(2), track(3)];

  it('steps and clamps', () => {
    expect(moveSelection(v, 2, 1)).toBe(3);
    expect(moveSelection(v, 3, 5)).toBe(3);
    expect(moveSelection(v, 2, -9)).toBe(1);
  });

  it('starts at the top going down and at the bottom going up', () => {
    expect(moveSelection(v, null, 1)).toBe(1);
    expect(moveSelection(v, 99, -1)).toBe(3);
    expect(moveSelection([], 1, 1)).toBeNull();
  });
});

describe('small helpers', () => {
  it('flips the direction on the same column', () => {
    expect(nextSort({ key: 'bpm', dir: 'asc' }, 'bpm')).toEqual({ key: 'bpm', dir: 'desc' });
    expect(nextSort({ key: 'bpm', dir: 'desc' }, 'title')).toEqual({ key: 'title', dir: 'asc' });
  });

  it('loads into the first empty deck', () => {
    expect(targetDeck({ a: false, b: true })).toBe('b');
    expect(targetDeck({ a: true, b: true })).toBe('a');
    expect(targetDeck({ a: false, b: false })).toBe('a');
  });

  it('describes progress only while something runs', () => {
    expect(progressText({ importing: false, queued: 40, analyzed: 12 })).toBe(
      'Analysiere 12 von 40',
    );
    expect(progressText({ importing: true, queued: 0, analyzed: 0 })).toBe('Importiere …');
    expect(progressText({ importing: false, queued: 40, analyzed: 40 })).toBeNull();
  });

  it('formats duration and tempo', () => {
    expect(formatDuration(201.6)).toBe('3:22');
    expect(formatDuration(null)).toBe('–');
    expect(formatBpm(127.95)).toBe('128.0');
    expect(formatBpm(null)).toBe('–');
  });
});

describe('initial fetch and filter changes', () => {
  it('lets rows from events win over an older fetch', () => {
    const fromEvent = [track(2, { analysis: 'done', bpm: 128 })];
    const fetched = [track(1), track(2, { analysis: 'pending', bpm: null })];
    const merged = mergeUnder(fromEvent, fetched);
    expect(merged.map((t) => [t.id, t.analysis])).toEqual([
      [1, 'done'],
      [2, 'done'],
    ]);
    expect(merged[1]?.bpm).toBe(128);
  });

  it('moves a selection the filter hides onto the first shown row', () => {
    const base = {
      ...initialLibrary,
      tracks: [
        track(1, { title: 'Alpha' }),
        track(2, { title: 'Beta' }),
        track(3, { title: 'Bravo' }),
      ],
      selected: 1,
    };
    expect(keepVisibleSelection({ ...base, query: 'b' }).selected).toBe(2);
    const kept = { ...base, query: 'alpha' };
    expect(keepVisibleSelection(kept)).toBe(kept);
    expect(keepVisibleSelection({ ...base, query: 'zzz' }).selected).toBeNull();
  });
});
