import { store } from '../state/store';
import { clampZoom } from './peaks';

/** Set the detail waveform zoom step (shared by both decks). */
export function setZoom(z: number) {
  store.update((s) => {
    const waveZoom = clampZoom(z);
    return waveZoom === s.waveZoom ? s : { ...s, waveZoom };
  });
}

/** Wheel travel (in pixels) for one zoom step. A mouse notch is ~100 px; a trackpad swipe
 *  sends dozens of small deltas that now add up instead of each being a step. */
export const WHEEL_STEP = 80;

/** Add a wheel event to the running total: whole steps out, the remainder carries over. A
 *  turn in the other direction starts afresh. `deltaMode` 1 = lines, 2 = pages. */
export function wheelSteps(
  acc: number,
  delta: number,
  deltaMode = 0,
): { steps: number; rest: number } {
  const px = delta * (deltaMode === 1 ? 16 : deltaMode === 2 ? 400 : 1);
  if (!Number.isFinite(px)) return { steps: 0, rest: acc };
  const total = Math.sign(px) !== Math.sign(acc) ? px : acc + px;
  const steps = Math.trunc(total / WHEEL_STEP) || 0; // no -0
  return { steps, rest: total - steps * WHEEL_STEP };
}
