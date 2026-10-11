// Drag a library row onto a deck with pointer events (not HTML5 drag and drop: that would
// compete with the native Finder drop, and pointer events are testable with Playwright).
import { useCallback, useEffect, useRef, useState } from 'react';
import type { Deck, LibraryTrack } from '../ipc/types';

/** Pointer travel before a press becomes a drag; below it, it stays a click. */
const DRAG_THRESHOLD = 6;

export interface DragState {
  track: LibraryTrack;
  x: number;
  y: number;
  over: Deck | null;
}

function deckAt(x: number, y: number): Deck | null {
  const deck = document.elementFromPoint(x, y)?.closest<HTMLElement>('section[data-deck]')
    ?.dataset.deck;
  return deck === 'a' || deck === 'b' ? deck : null;
}

/** Marks the deck under the pointer so it can light up as a drop target. */
function markTarget(deck: Deck | null) {
  document.querySelectorAll<HTMLElement>('section[data-deck]').forEach((el) => {
    if (el.dataset.deck === deck) el.dataset.dropTarget = '';
    else delete el.dataset.dropTarget;
  });
}

export function useTrackDrag(onDrop: (deck: Deck, track: LibraryTrack) => void) {
  const [drag, setDrag] = useState<DragState | null>(null);
  const pending = useRef<{ track: LibraryTrack; x: number; y: number; id: number } | null>(null);
  const active = useRef(false);
  const dropRef = useRef(onDrop);
  useEffect(() => {
    dropRef.current = onDrop;
  }, [onDrop]);

  useEffect(() => {
    const move = (e: PointerEvent) => {
      const p = pending.current;
      if (!p || e.pointerId !== p.id) return;
      if (!active.current && Math.hypot(e.clientX - p.x, e.clientY - p.y) < DRAG_THRESHOLD) {
        return;
      }
      active.current = true;
      const over = deckAt(e.clientX, e.clientY);
      markTarget(over);
      setDrag({ track: p.track, x: e.clientX, y: e.clientY, over });
    };
    const end = (e: PointerEvent) => {
      const p = pending.current;
      if (!p || e.pointerId !== p.id) return;
      pending.current = null;
      if (active.current && e.type === 'pointerup') {
        const deck = deckAt(e.clientX, e.clientY);
        if (deck) dropRef.current(deck, p.track);
      }
      active.current = false;
      markTarget(null);
      setDrag(null);
    };
    const cancel = (e: KeyboardEvent) => {
      if (e.key !== 'Escape' || !pending.current) return;
      pending.current = null;
      active.current = false;
      markTarget(null);
      setDrag(null);
    };
    // On the window: the row may scroll out of the list while dragging.
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', end);
    window.addEventListener('pointercancel', end);
    window.addEventListener('keydown', cancel);
    return () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', end);
      window.removeEventListener('pointercancel', end);
      window.removeEventListener('keydown', cancel);
    };
  }, []);

  const start = useCallback((e: React.PointerEvent, track: LibraryTrack) => {
    if (e.button !== 0) return;
    pending.current = { track, x: e.clientX, y: e.clientY, id: e.pointerId };
  }, []);

  return { drag, start };
}
