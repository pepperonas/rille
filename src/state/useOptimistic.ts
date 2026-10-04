import { useEffect, useRef, useState } from 'react';

/** Values within this distance count as "the engine confirmed what we sent". */
const CONFIRM_EPSILON = 1e-3;

/**
 * How long a value sent to the engine is shown without confirmation. The engine echoes within
 * one frame (~16 ms); this only matters when it never echoes a *change* — e.g. two quick
 * toggles that end where they started. Without the timeout the stale value would hide every
 * later change made elsewhere (controller).
 */
export const PENDING_TIMEOUT_MS = 500;

/**
 * A value the UI changes faster than the engine confirms it (keyboard repeat, wheel bursts,
 * double clicks). Shows and steps from the last value sent until the engine reports it back,
 * so consecutive steps accumulate instead of all starting from the same stale snapshot.
 */
export function useOptimistic<T extends number | boolean>(confirmed: T) {
  const [pending, setPending] = useState<T | null>(null);
  const pendingRef = useRef<T | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const clear = () => {
    if (timer.current) clearTimeout(timer.current);
    timer.current = null;
    pendingRef.current = null;
    setPending(null);
  };

  useEffect(() => {
    const p = pendingRef.current;
    if (p === null) return;
    const same =
      typeof p === 'number' && typeof confirmed === 'number'
        ? Math.abs(p - confirmed) < CONFIRM_EPSILON
        : p === confirmed;
    if (same) clear();
  }, [confirmed]);

  useEffect(
    () => () => {
      if (timer.current) clearTimeout(timer.current);
    },
    [],
  );

  const current = (): T => pendingRef.current ?? confirmed;
  const set = (value: T) => {
    pendingRef.current = value;
    setPending(value);
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(clear, PENDING_TIMEOUT_MS);
  };
  return { shown: pending ?? confirmed, current, set };
}
