import type { DeckFrame, TempoRange } from '../ipc/types';

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
): number {
  if (!deck.playing || sampleRate <= 0 || !Number.isFinite(deck.rate)) return deck.position;
  const elapsed = Math.min(Math.max(now - receivedAt, 0), MAX_EXTRAPOLATION_MS) / 1000;
  const pos = deck.position + elapsed * sampleRate * deck.rate;
  return Math.min(deck.frames, Math.max(0, pos));
}

/** Span of each tempo range as a fraction (0.1 = ±10 %). Mirrors `TempoRange::span`. */
export const RANGE_SPAN: Record<TempoRange, number> = {
  six: 0.06,
  ten: 0.1,
  sixteen: 0.16,
  wide: 0.5,
};

/** Order of the range chip, as on the controller (Shift + Sync). */
export const NEXT_RANGE: Record<TempoRange, TempoRange> = {
  six: 'ten',
  ten: 'sixteen',
  sixteen: 'wide',
  wide: 'six',
};

/** Tempo change in percent for a fader position, e.g. `+2.40 %`. */
export function formatTempo(tempo: number, range: TempoRange): string {
  const pct = (Number.isFinite(tempo) ? Math.max(-1, Math.min(1, tempo)) : 0) * RANGE_SPAN[range] * 100;
  const rounded = Math.round(pct * 100) / 100;
  if (rounded === 0) return '0.00 %';
  return `${rounded > 0 ? '+' : '−'}${Math.abs(rounded).toFixed(2)} %`;
}

/** Fader value (top = 1) → tempo (top = −1, slower), as printed on the DDJ-200. */
export const faderToTempo = (value: number) => 1 - 2 * Math.max(0, Math.min(1, value));

/** Tempo → fader value; the inverse of `faderToTempo`. */
export const tempoToFader = (tempo: number) =>
  Number.isFinite(tempo) ? (1 - Math.max(-1, Math.min(1, tempo))) / 2 : 0.5;

/** Range chip label, e.g. `±10 %`. */
export function formatRange(range: TempoRange): string {
  return range === 'wide' ? 'WIDE' : `±${Math.round(RANGE_SPAN[range] * 100)} %`;
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
