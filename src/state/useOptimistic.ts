import { useEffect, useRef, useState } from 'react';

/** Values within this distance count as "the engine confirmed what we sent". */
const CONFIRM_EPSILON = 1e-3;

/**
 * A value the UI changes faster than the engine confirms it (keyboard repeat, wheel bursts,
 * double clicks). Shows and steps from the last value sent until the engine reports it back,
 * so consecutive steps accumulate instead of all starting from the same stale snapshot.
 */
export function useOptimistic<T extends number | boolean>(confirmed: T) {
  const [pending, setPending] = useState<T | null>(null);
  const pendingRef = useRef<T | null>(null);

  useEffect(() => {
    const p = pendingRef.current;
    if (p === null) return;
    const same =
      typeof p === 'number' && typeof confirmed === 'number'
        ? Math.abs(p - confirmed) < CONFIRM_EPSILON
        : p === confirmed;
    if (same) {
      pendingRef.current = null;
      setPending(null);
    }
  }, [confirmed]);

  const current = (): T => pendingRef.current ?? confirmed;
  const set = (value: T) => {
    pendingRef.current = value;
    setPending(value);
  };
  return { shown: pending ?? confirmed, current, set };
}
