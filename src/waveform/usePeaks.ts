// Peaks of the track on a deck, fetched once the library reports its analysis as done.
import { useEffect, useState } from 'react';
import { backend } from '../ipc';
import { trackById } from '../library/model';
import { useAppState } from '../state/store';
import { parsePeaks, type Peaks } from './peaks';

/** Keyed by library id and analysis revision: a re-analysis never reuses old peaks. */
const cache = new Map<string, Promise<Peaks>>();
/** Cached waveforms kept at most (a few decks' worth of history). */
const CACHE_LIMIT = 16;

function fetchPeaks(id: number, rev: number): Promise<Peaks> {
  const key = `${id}:${rev}`;
  let p = cache.get(key);
  if (!p) {
    p = backend.libraryPeaks(id).then(parsePeaks);
    const mine = p;
    // A failed fetch may be retried later – but only drop our own entry.
    p.catch(() => {
      if (cache.get(key) === mine) cache.delete(key);
    });
    cache.set(key, p);
    while (cache.size > CACHE_LIMIT) {
      const oldest = cache.keys().next().value;
      if (oldest === undefined) break;
      cache.delete(oldest);
    }
  }
  return p;
}

export type PeaksState =
  | { status: 'ready'; peaks: Peaks }
  /** Analysis queued or running. */
  | { status: 'waiting' }
  /** No waveform will come: analysis failed, or no library. */
  | { status: 'unavailable'; reason: string };

export function usePeaks(libraryId: number | null): PeaksState {
  const track = useAppState((s) => trackById(s.library.tracks, libraryId));
  const analysis = track?.analysis;
  const rev = track?.analysisRev ?? 0;
  const [loaded, setLoaded] = useState<{ key: string; peaks: Peaks } | null>(null);
  const [failed, setFailed] = useState<{ key: string; reason: string } | null>(null);
  const key = `${libraryId}:${rev}`;

  useEffect(() => {
    if (libraryId === null || analysis !== 'done') return;
    let live = true;
    fetchPeaks(libraryId, rev).then(
      (peaks) => live && setLoaded({ key: `${libraryId}:${rev}`, peaks }),
      (e: unknown) =>
        live &&
        setFailed({
          key: `${libraryId}:${rev}`,
          reason: e instanceof Error ? e.message : String(e),
        }),
    );
    return () => {
      live = false;
    };
  }, [libraryId, analysis, rev]);

  if (libraryId === null) return { status: 'unavailable', reason: 'Keine Waveform ohne Library' };
  if (analysis === 'failed') {
    return { status: 'unavailable', reason: track?.error ?? 'Analyse fehlgeschlagen' };
  }
  if (analysis === 'done' && loaded?.key === key) return { status: 'ready', peaks: loaded.peaks };
  if (analysis === 'done' && failed?.key === key) {
    return { status: 'unavailable', reason: failed.reason };
  }
  return { status: 'waiting' };
}
