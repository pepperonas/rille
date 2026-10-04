import type { DeckFrame } from '../ipc/types';

/** How far past the last frame we keep extrapolating before assuming the stream stalled. */
export const MAX_EXTRAPOLATION_MS = 250;

/**
 * Playback position at `now`, extrapolated from the last state frame. Frames arrive at
 * ≤ 60 Hz; waveforms and clocks draw at display rate (up to 120 Hz) from this.
 */
export function positionAt(
  deck: DeckFrame,
  sampleRate: number,
  receivedAt: number,
  now: number,
  rate = 1,
): number {
  if (!deck.playing || sampleRate <= 0) return deck.position;
  const elapsed = Math.min(Math.max(now - receivedAt, 0), MAX_EXTRAPOLATION_MS) / 1000;
  return Math.min(deck.frames, deck.position + elapsed * sampleRate * rate);
}

/** `m:ss.t` for elapsed time; tenths stay so the readout visibly runs. */
export function formatTime(frames: number, sampleRate: number): string {
  if (sampleRate <= 0 || !Number.isFinite(frames)) return '0:00.0';
  const tenths = Math.floor((Math.max(0, frames) / sampleRate) * 10);
  const minutes = Math.floor(tenths / 600);
  const seconds = Math.floor((tenths % 600) / 10);
  return `${minutes}:${String(seconds).padStart(2, '0')}.${tenths % 10}`;
}

/** Remaining time with a leading minus, rounded up so it reaches 0:00.0 exactly at the end. */
export function formatRemaining(position: number, frames: number, sampleRate: number): string {
  if (sampleRate <= 0) return '−0:00.0';
  const left = Math.max(0, frames - position);
  const tenths = Math.ceil((left / sampleRate) * 10);
  return `−${formatTime((tenths / 10) * sampleRate + 0.5, sampleRate)}`;
}
