// Peaks of the track on a deck, fetched once the library reports its analysis as done.
import { useEffect, useState } from 'react';
import { backend } from '../ipc';
import { trackById } from '../library/model';
import { useAppState } from '../state/store';
import { parsePeaks, type Peaks } from './peaks';

const cache = new Map<number, Promise<Peaks>>();

function fetchPeaks(id: number): Promise<Peaks> {
  let p = cache.get(id);
  if (!p) {
    p = backend.libraryPeaks(id).then(parsePeaks);
    // A failed fetch (still analysing, cache cleared) may be retried later.
    p.catch(() => cache.delete(id));
    cache.set(id, p);
  }
  return p;
}

/** `null` while there is nothing to draw (no library track, analysis running, error). */
export function usePeaks(libraryId: number | null): Peaks | null {
  const analysis = useAppState((s) => trackById(s.library.tracks, libraryId)?.analysis);
  const [loaded, setLoaded] = useState<{ id: number; peaks: Peaks } | null>(null);

  useEffect(() => {
    if (libraryId === null) return;
    // Analysed again (file changed, cache cleared): the old peaks are stale.
    if (analysis === 'pending') cache.delete(libraryId);
    if (analysis !== 'done') return;
    let live = true;
    fetchPeaks(libraryId).then(
      (peaks) => live && setLoaded({ id: libraryId, peaks }),
      () => {},
    );
    return () => {
      live = false;
    };
  }, [libraryId, analysis]);

  return loaded && loaded.id === libraryId && analysis === 'done' ? loaded.peaks : null;
}
